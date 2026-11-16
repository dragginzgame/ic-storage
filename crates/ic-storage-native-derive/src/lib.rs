//! Experimental scalar declarations. This macro never generates unsafe code or
//! grants admission to retained memory. See the crate README for the subset.
#![forbid(unsafe_code)]

use proc_macro::TokenStream;

mod expand;

/// Generate metadata, a bounded logical codec and a scoped scalar view.
/// Requires repr(C), Copy, named u32/u64 fields with explicit IDs, and an explicit
/// eight-byte schema ID and nonzero version. Owning fields and Drop are excluded.
#[proc_macro_derive(Store, attributes(store))]
pub fn derive_store(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    expand::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
