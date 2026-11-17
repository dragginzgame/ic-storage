use super::{Scalar, expand};
use syn::{DeriveInput, parse_quote};

#[test]
fn scalar_paths_are_exact_and_do_not_admit_aliases_or_type_arguments() {
    for ty in [
        parse_quote!(u32),
        parse_quote!(u64),
        parse_quote!(::core::primitive::u32),
        parse_quote!(::core::primitive::u64),
    ] {
        assert!(Scalar::parse(&ty).is_ok());
    }
    for ty in [
        parse_quote!(::u32),
        parse_quote!(core::primitive::u32),
        parse_quote!(application::u32),
        parse_quote!(::core::primitive::usize),
        parse_quote!(::core::primitive::u32::Associated),
        parse_quote!(::core::primitive::u32<u64>),
        parse_quote!(::core::primitive::<u32>::u64),
        parse_quote!(<Record as Trait>::u32),
        parse_quote!(&::core::primitive::u32),
        parse_quote!([::core::primitive::u32; 1]),
    ] {
        assert!(Scalar::parse(&ty).is_err());
    }
}

#[test]
fn bounded_scalar_declaration_accepts_explicit_identity_and_reordered_fields() {
    let input = parse_quote! {
        #[repr(C)]
        #[store(id = "RECU64V1", version = 2, validate = Self::valid)]
        struct Value {
            #[store(field = 9)] ceiling: u64,
            #[store(field = 1)] counter: u32,
        }
    };
    assert!(expand(&input).is_ok());
}

#[test]
fn unsupported_shapes_and_ambiguous_options_refuse_expansion() {
    let inputs: Vec<DeriveInput> = vec![
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] enum Value { A } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value(u64); },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value<T> { #[store(field=1)] data: T } },
        parse_quote! { #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] data: u64 } },
        parse_quote! { #[repr(C, packed)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] data: u64 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=0)] struct Value { #[store(field=1)] data: u64 } },
        parse_quote! { #[repr(C)] #[store(id="bad", version=1)] struct Value { #[store(field=1)] data: u64 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1, version=2)] struct Value { #[store(field=1)] data: u64 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { data: u64 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=0)] data: u64 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] a: u64, #[store(field=1)] b: u64 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1, skip)] data: u64 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] data: Vec<u64> } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] data: &'static u64 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] data: fn() } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] data: i32 } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] data: usize } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value { #[store(field=1)] data: bool } },
        parse_quote! { #[repr(C)] #[store(id="RECU64V1", version=1)] struct Value {} },
    ];
    for input in inputs {
        assert!(expand(&input).is_err(), "accepted {}", input.ident);
    }
}

#[test]
fn field_count_bound_is_enforced_before_generation() {
    let mut input: DeriveInput = parse_quote! {
        #[repr(C)] #[store(id = "BOUNDARY", version = 1)] struct Value {}
    };
    let syn::Data::Struct(data) = &mut input.data else {
        unreachable!()
    };
    let syn::Fields::Named(fields) = &mut data.fields else {
        unreachable!()
    };
    for number in 1u32..=16 {
        let name = quote::format_ident!("field_{number}");
        fields
            .named
            .push(parse_quote! { #[store(field = #number)] #name: u64 });
    }
    assert!(expand(&input).is_ok());
    let syn::Data::Struct(data) = &mut input.data else {
        unreachable!()
    };
    let syn::Fields::Named(fields) = &mut data.fields else {
        unreachable!()
    };
    fields
        .named
        .push(parse_quote! { #[store(field = 17)] extra: u64 });
    assert!(expand(&input).is_err());
}
