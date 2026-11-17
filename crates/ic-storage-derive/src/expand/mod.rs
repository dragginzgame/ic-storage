use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitInt, LitStr, Path, Type};

#[derive(Clone, Copy)]
enum Scalar {
    U32,
    U64,
}

impl Scalar {
    fn parse(ty: &Type) -> syn::Result<Self> {
        let fail = || {
            syn::Error::new_spanned(
                ty,
                "Store supports only u32/u64 fields (bare or ::core::primitive paths)",
            )
        };
        let Type::Path(ty) = ty else {
            return Err(fail());
        };
        if ty.qself.is_some() {
            return Err(fail());
        }
        let segments = &ty.path.segments;
        let bare = ty.path.leading_colon.is_none() && segments.len() == 1;
        let absolute_primitive = ty.path.leading_colon.is_some()
            && segments.len() == 3
            && segments[0].ident == "core"
            && segments[1].ident == "primitive";
        if !(bare || absolute_primitive)
            || segments.iter().any(|segment| !segment.arguments.is_empty())
        {
            return Err(fail());
        }
        match segments.last() {
            Some(segment) if segment.ident == "u32" => Ok(Self::U32),
            Some(segment) if segment.ident == "u64" => Ok(Self::U64),
            _ => Err(fail()),
        }
    }

    fn width(self) -> usize {
        match self {
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }

    fn primitive(self) -> TokenStream {
        match self {
            Self::U32 => quote!(::core::primitive::u32),
            Self::U64 => quote!(::core::primitive::u64),
        }
    }

    fn metadata(self) -> TokenStream {
        match self {
            Self::U32 => quote!(::ic_storage::record::Scalar::U32),
            Self::U64 => quote!(::ic_storage::record::Scalar::U64),
        }
    }
}

pub(super) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let fail = |message| syn::Error::new_spanned(input, message);
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(fail("Store does not support generic records"));
    }
    let repr: Vec<_> = input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("repr"))
        .collect();
    if repr.len() != 1 || !repr[0].parse_args::<Path>()?.is_ident("C") {
        return Err(fail("Store requires exactly #[repr(C)]"));
    }
    let mut id = None;
    let mut version = None;
    let mut validate: Option<Path> = None;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("store"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("id") && id.is_none() {
                id = Some(meta.value()?.parse::<LitStr>()?);
            } else if meta.path.is_ident("version") && version.is_none() {
                version = Some(meta.value()?.parse::<LitInt>()?.base10_parse::<u32>()?);
            } else if meta.path.is_ident("validate") && validate.is_none() {
                validate = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("unknown or duplicate Store option"));
            }
            Ok(())
        })?;
    }
    let id = id.ok_or_else(|| fail("Store requires an explicit id"))?;
    let id_value = id.value();
    if id_value.len() != 8 || !id_value.is_ascii() || id_value.as_bytes().contains(&0) {
        return Err(syn::Error::new_spanned(
            id,
            "Store id must be eight non-NUL ASCII bytes",
        ));
    }
    let version = version
        .filter(|version| *version != 0)
        .ok_or_else(|| fail("Store requires a nonzero u32 version"))?;
    let Data::Struct(data) = &input.data else {
        return Err(fail("Store requires a named-field struct"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(fail("Store requires named fields"));
    };
    if fields.named.is_empty() || fields.named.len() > 16 {
        return Err(fail("Store supports one to sixteen u32/u64 fields"));
    }
    let mut ordered = Vec::new();
    for field in &fields.named {
        let scalar = Scalar::parse(&field.ty)?;
        let mut number = None;
        for attr in field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("store"))
        {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("field") && number.is_none() {
                    number = Some(meta.value()?.parse::<LitInt>()?.base10_parse::<u32>()?);
                    Ok(())
                } else {
                    Err(meta.error("unknown or duplicate Store field option"))
                }
            })?;
        }
        let number = number
            .filter(|number| *number != 0)
            .ok_or_else(|| syn::Error::new_spanned(field, "Store requires a nonzero field ID"))?;
        ordered.push((number, field.ident.as_ref().expect("named field"), scalar));
    }
    ordered.sort_unstable_by_key(|(number, _, _)| *number);
    if ordered.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(fail("Store field IDs must be unique"));
    }
    let name = &input.ident;
    let vis = &input.vis;
    let view = format_ident!("{}View", name);
    let field_names: Vec<_> = ordered.iter().map(|(_, name, _)| *name).collect();
    let field_ids: Vec<_> = ordered.iter().map(|(number, _, _)| number).collect();
    let primitives: Vec<_> = ordered
        .iter()
        .map(|(_, _, scalar)| scalar.primitive())
        .collect();
    let kinds: Vec<_> = ordered
        .iter()
        .map(|(_, _, scalar)| scalar.metadata())
        .collect();
    let widths: Vec<_> = ordered
        .iter()
        .map(|(_, _, scalar)| scalar.width())
        .collect();
    let count = ordered.len();
    let mut length = 0;
    let mut starts = Vec::with_capacity(count);
    let mut ends = Vec::with_capacity(count);
    for width in &widths {
        starts.push(length);
        length += width;
        ends.push(length);
    }
    let schema = syn::LitByteStr::new(id_value.as_bytes(), id.span());
    let validate = validate.map_or_else(|| quote!(true), |path| quote!(#path(self)));
    Ok(quote! {
        // Copy excludes user Drop even when all declared fields are scalars.
        const _: () = {
            fn require_copy<T: ::core::marker::Copy>() {}
            let _ = require_copy::<#name>;
        };
        impl #name {
            /// Owner-assigned logical identity; not a runtime admission token.
            pub const STORE_ID: &'static [::core::primitive::u8; 8] = #schema;
            /// Explicit application schema version.
            pub const STORE_VERSION: ::core::primitive::u32 = #version;
            /// Exact bounded logical payload size.
            pub const STORE_BYTES: ::core::primitive::usize = #length;
            /// Size of this concrete repr(C) payload, not a private-layout certificate.
            pub const STORE_SIZE: ::core::primitive::usize = ::core::mem::size_of::<Self>();
            /// Alignment of this concrete repr(C) payload.
            pub const STORE_ALIGN: ::core::primitive::usize = ::core::mem::align_of::<Self>();
            /// Scalar field IDs and native offsets, ordered by field identity.
            pub const STORE_FIELDS: [::ic_storage::record::Field; #count] = [
                #(::ic_storage::record::Field {
                    id: #field_ids, offset: ::core::mem::offset_of!(Self, #field_names),
                    scalar: #kinds,
                }),*
            ];
            fn store_valid(&self) -> ::core::primitive::bool { #validate }
            /// Encode only logical fields in field-ID order, each little endian.
            /// Schema identity/version belong to the enclosing checked envelope.
            pub fn store_encode(&self) -> ::core::result::Result<[::core::primitive::u8; #length], ::ic_storage::record::Error> {
                if !self.store_valid() { return ::core::result::Result::Err(::ic_storage::record::Error::Invariant); }
                let mut bytes = [0; #length];
                #(bytes[#starts..#ends].copy_from_slice(&self.#field_names.to_le_bytes());)*
                ::core::result::Result::Ok(bytes)
            }
            /// Decode an exact-size logical payload and validate application rules.
            /// This constructs a fresh value; it never reopens native memory.
            pub fn store_decode(bytes: &[::core::primitive::u8]) -> ::core::result::Result<Self, ::ic_storage::record::Error> {
                if bytes.len() != #length { return ::core::result::Result::Err(::ic_storage::record::Error::Length); }
                let value = Self {
                    #(#field_names: {
                        let mut word = [0; #widths];
                        word.copy_from_slice(&bytes[#starts..#ends]);
                        #primitives::from_le_bytes(word)
                    }),*
                };
                if !value.store_valid() { return ::core::result::Result::Err(::ic_storage::record::Error::Invariant); }
                ::core::result::Result::Ok(value)
            }
            /// Lend a scoped view of an already valid record, with scalar getters.
            /// Callers own admission and poison/trap policy; errors do not undo writes.
            pub fn store_with<R>(&mut self, operation: impl for<'scope> ::core::ops::FnOnce(#view<'scope>) -> R) -> R {
                operation(#view { record: self })
            }
        }
        /// Scoped scalar view. No owning field, mutable record or Deref is exposed.
        #vis struct #view<'scope> { record: &'scope mut #name }
        impl #view<'_> {
            #(
                #[doc = ::core::concat!("Copy the scalar field `", ::core::stringify!(#field_names), "`.")]
                #[must_use]
                pub fn #field_names(&self) -> #primitives { self.record.#field_names }
            )*
        }
    })
}

#[cfg(test)]
mod tests;
