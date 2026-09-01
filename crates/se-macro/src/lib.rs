//! Proc macros behind the `se` facade. Nothing here is meant to be used
//! directly — depend on `se` and reach them as `se::Schema` / `#[se::stage]`.

use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, DeriveInput, Ident, ItemFn, Token};

mod schema;
mod stage;

/// Describe a `#[repr(C)]` struct to the host so it can be stored, saved and
/// fed to a render pass without the host ever knowing the Rust type.
#[proc_macro_derive(Schema)]
pub fn derive_schema(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    schema::derive(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Make a function a stage: its parameter list becomes its query.
#[proc_macro_attribute]
pub fn stage(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[se::stage] takes no arguments — the signature is the whole spec",
        )
        .into_compile_error()
        .into();
    }
    let f = parse_macro_input!(item as ItemFn);
    stage::stage(f)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// `se::stages!(motion, collide);` — the `process/*.so` entry point.
///
/// A separate macro from the others only because it has to name the hidden
/// module `#[se::stage]` generated, and `macro_rules!` cannot build an
/// identifier.
#[proc_macro]
pub fn stages(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let names =
        parse_macro_input!(input with Punctuated::<Ident, Token![,]>::parse_terminated);
    let specs = names.iter().map(|n| {
        let m = format_ident!("__se_stage_{}", n);
        quote!(sink.push(&#m::SPEC);)
    });
    quote! {
        ::se::abi_version!();

        /// # Safety
        /// Called by the host with a sink it owns.
        #[no_mangle]
        pub unsafe extern "C" fn se_register_stages(sink: *mut ::se::StageSink) {
            let sink = &mut *sink;
            #(#specs)*
        }
    }
    .into()
}
