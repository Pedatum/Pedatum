//! `#[se::stage]` — turn a plain function into a queryable stage.
//!
//! The parameter list is the query, so the macro's whole job is to read the
//! signature and emit a batched `extern "C"` shim around it. Note what it
//! cannot emit: there is no way to name a component the signature does not,
//! which is what makes "reaching outside the signature is not expressible"
//! true rather than aspirational.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Error, FnArg, ItemFn, Result, Type};

enum Binding {
    /// `&T`
    Ref(Type),
    /// `&mut T`
    Mut(Type),
    /// `T` by value — only `f32`, meaning dt.
    Value(Type),
}

fn classify(arg: &FnArg) -> Result<Binding> {
    let pat = match arg {
        FnArg::Typed(t) => t,
        FnArg::Receiver(r) => {
            return Err(Error::new(r.span(), "a stage is a free function, not a method"))
        }
    };
    Ok(match &*pat.ty {
        Type::Reference(r) => {
            let inner = (*r.elem).clone();
            if r.mutability.is_some() {
                Binding::Mut(inner)
            } else {
                Binding::Ref(inner)
            }
        }
        other => Binding::Value(other.clone()),
    })
}

pub fn stage(f: ItemFn) -> Result<TokenStream> {
    let name = f.sig.ident.clone();
    let name_str = name.to_string();
    let modname = format_ident!("__se_stage_{}", name);

    if f.sig.asyncness.is_some() || f.sig.unsafety.is_some() {
        return Err(Error::new(
            f.sig.span(),
            "a stage is an ordinary safe function; the host owns scheduling",
        ));
    }
    if !matches!(f.sig.output, syn::ReturnType::Default) {
        return Err(Error::new(
            f.sig.output.span(),
            "a stage writes through its parameters and returns nothing",
        ));
    }

    let bindings: Vec<Binding> = f.sig.inputs.iter().map(classify).collect::<Result<_>>()?;
    if bindings.is_empty() {
        return Err(Error::new(
            f.sig.span(),
            "a stage with no parameters has an empty query and nothing to do",
        ));
    }

    let mut param_consts = Vec::new();
    let mut slot_consts = Vec::new();
    let mut args = Vec::new();

    for (i, b) in bindings.iter().enumerate() {
        let ty = match b {
            Binding::Ref(t) | Binding::Mut(t) | Binding::Value(t) => t,
        };
        param_consts.push(match b {
            Binding::Mut(_) => quote!(<#ty as ::se::StageParam>::PARAM.write()),
            _ => quote!(<#ty as ::se::StageParam>::PARAM),
        });

        let slot = format_ident!("SLOT{}", i);
        slot_consts.push(quote! {
            const #slot: u32 = ::se::slot_of(&PARAMS, #i);
        });

        let fetch = quote! { <#ty as ::se::StageParam>::fetch(call, #slot, row) };
        args.push(match b {
            Binding::Mut(_) => quote! { &mut *(#fetch as *mut #ty) },
            Binding::Ref(_) => quote! { &*(#fetch as *const #ty) },
            Binding::Value(_) => quote! { *(#fetch as *const #ty) },
        });
    }

    let n = param_consts.len();
    let vis = &f.vis;

    Ok(quote! {
        #f

        #[doc(hidden)]
        #vis mod #modname {
            use super::*;

            const PARAMS: [::se::Param; #n] = [#(#param_consts),*];
            static PARAM_TABLE: [::se::Param; #n] = PARAMS;
            #(#slot_consts)*

            /// # Safety
            /// Called by the host with a call frame matching `PARAM_TABLE`.
            pub unsafe extern "C" fn run(call: *const ::se::StageCall) {
                let n_rows = (*call).n_rows;
                for row in 0..n_rows {
                    super::#name(#(#args),*);
                }
            }

            pub static SPEC: ::se::StageSpec = ::se::StageSpec {
                name: ::se::Str::new(#name_str),
                params: ::se::Slice::new(&PARAM_TABLE),
                run,
            };
        }
    })
}
