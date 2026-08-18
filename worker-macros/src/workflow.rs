use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ItemStruct;

pub fn expand_macro(attr: TokenStream, tokens: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[workflow] does not accept arguments",
        ));
    }

    let target = syn::parse2::<ItemStruct>(tokens)?;
    if !target.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &target.generics,
            "Workflow entrypoint structs cannot be generic",
        ));
    }

    let target_name = &target.ident;
    let marker_name = format_ident!("__worker_workflow_entrypoint_{}", target_name);

    Ok(quote! {
        #target

        impl ::worker::has_workflow_attribute for #target_name {}

        const _: () = {
            use ::worker::wasm_bindgen::prelude::*;
            #[allow(unused_imports)]
            use ::worker::WorkflowEntrypoint;

            #[wasm_bindgen(wasm_bindgen=::worker::wasm_bindgen)]
            #[::worker::consume]
            #target

            #[wasm_bindgen(wasm_bindgen=::worker::wasm_bindgen)]
            impl #target_name {
                #[wasm_bindgen(constructor, wasm_bindgen=::worker::wasm_bindgen)]
                pub fn new(
                    ctx: ::worker::worker_sys::Context,
                    env: ::worker::Env,
                ) -> Self {
                    <Self as ::worker::WorkflowEntrypoint>::new(
                        ::worker::Context::new(ctx),
                        env,
                    )
                }

                #[wasm_bindgen(js_name = run, wasm_bindgen=::worker::wasm_bindgen)]
                pub fn run(
                    &self,
                    event: ::worker::wasm_bindgen::JsValue,
                    step: ::worker::worker_sys::WorkflowStep,
                ) -> ::worker::js_sys::Promise {
                    // SAFETY: The Workflows runtime retains the entrypoint object while its run
                    // Promise is pending, so the Rust value cannot be destroyed during this future.
                    let static_self: &'static Self = unsafe { &*(self as *const _) };

                    ::worker::js_sys::futures::future_to_promise(
                        ::std::panic::AssertUnwindSafe(async move {
                            let event = ::worker::WorkflowEvent::<<Self as ::worker::WorkflowEntrypoint>::Input>::_from_raw(event)
                                .map_err(::worker::wasm_bindgen::JsValue::from)?;
                            let output = <Self as ::worker::WorkflowEntrypoint>::run(
                                static_self,
                                event,
                                ::worker::WorkflowStep::from(step),
                            )
                            .await
                            .map_err(::worker::wasm_bindgen::JsValue::from)?;

                            ::worker::serde_wasm_bindgen::to_value(&output)
                                .map_err(::worker::Error::from)
                                .map_err(::worker::wasm_bindgen::JsValue::from)
                        }),
                    )
                }
            }

            #[allow(non_snake_case)]
            #[wasm_bindgen(wasm_bindgen=::worker::wasm_bindgen)]
            pub fn #marker_name() {}
        };
    })
}
