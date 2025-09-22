use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

/// Representation of a single RPC method extracted from a trait.
struct RpcMethod {
    name: syn::Ident,        // e.g. "tlock_ping"
    method_enum: syn::Path,  // e.g. Methods::TlockPing
    args: Vec<syn::PatType>, // some arguments
    output: syn::Type,       // Result<Ret, Error>
}

/// Representation of an RPC trait we’re generating clients/servers for.
struct RpcNamespace {
    trait_name: syn::Ident,  // e.g. GlobalNamespace
    client_name: syn::Ident, // e.g. GlobalNamespaceClient
    server_name: syn::Ident, // e.g. GlobalNamespaceServer
    methods: Vec<RpcMethod>,
}

#[proc_macro_attribute]
pub fn rpc_namespace(_args: TokenStream, input: TokenStream) -> TokenStream {
    let trait_ast = parse_macro_input!(input as syn::ItemTrait);

    let ns = match parse_trait(&trait_ast) {
        Ok(ns) => ns,
        Err(e) => return e.to_compile_error().into(),
    };

    let expanded = expand_namespace(ns, &trait_ast);
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn rpc_method(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

fn parse_trait(item: &syn::ItemTrait) -> syn::Result<RpcNamespace> {
    let trait_name = item.ident.clone();
    let client_name = syn::Ident::new(&format!("{}Client", trait_name), trait_name.span());
    let server_name = syn::Ident::new(&format!("{}Server", trait_name), trait_name.span());

    let mut methods = Vec::new();

    for item in &item.items {
        if let syn::TraitItem::Fn(m) = item {
            let sig = &m.sig;
            let name = sig.ident.clone();

            // Require #[rpc_method(...)]
            let method_attr = m
                .attrs
                .iter()
                .find(|a| a.path().is_ident("rpc_method"))
                .ok_or_else(|| {
                    syn::Error::new_spanned(&m.sig.ident, "missing #[rpc_method(...)] attribute")
                })?;
            let method_enum: syn::Path = method_attr.parse_args()?;

            // Skip &self
            let mut inputs = sig.inputs.iter().skip(1);
            let mut args = Vec::new();
            while let Some(arg) = inputs.next() {
                match arg {
                    syn::FnArg::Typed(pat) => args.push((pat).clone()),
                    syn::FnArg::Receiver(_) => {
                        return Err(syn::Error::new_spanned(arg, "unexpected receiver"));
                    }
                }
            }

            // Output type
            let output = match &sig.output {
                syn::ReturnType::Type(_, ty) => (**ty).clone(),
                syn::ReturnType::Default => {
                    return Err(syn::Error::new_spanned(
                        &sig.ident,
                        "method must return Result",
                    ));
                }
            };

            methods.push(RpcMethod {
                name,
                method_enum,
                args,
                output,
            });
        }
    }

    Ok(RpcNamespace {
        trait_name,
        client_name,
        server_name,
        methods,
    })
}

fn expand_namespace(ns: RpcNamespace, original: &syn::ItemTrait) -> proc_macro2::TokenStream {
    let RpcNamespace {
        trait_name,
        client_name,
        server_name,
        methods,
    } = ns;

    // ---- Client impl ----
    let client_methods = methods.iter().map(|m| {
        let RpcMethod {
            name,
            method_enum,
            args, // Vec<Arg> { pat: syn::Pat, ty: syn::Type }
            output,
        } = m;

        // split out patterns and types
        let arg_pats: Vec<_> = args.iter().map(|a| &a.pat).collect();
        let arg_tys: Vec<_> = args.iter().map(|a| &a.ty).collect();

        if arg_pats.is_empty() {
            // 0-arg path: send [] (empty positional params)
            quote! {
                async fn #name(&self) -> #output {
                    let req = ::serde_json::Value::Array(::std::vec::Vec::new());
                    let resp = self.transport
                        .call(&#method_enum.to_string(), req)
                        .await?;
                    let resp = ::serde_json::from_value(resp.result)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;
                    Ok(resp)
                }
            }
        } else {
            // 1+ args path: (T1, T2, ..., Tn) and (a1, a2, ..., an)
            // `#(#x,)*` ensures the single-element tuple has a trailing comma.
            quote! {
                async fn #name(&self, #( #args ),* ) -> #output {
                    type __Params = ( #( #arg_tys, )* );
                    let __params: __Params = ( #( #arg_pats, )* );
                    let __params = ::wasmi_pdk::flex_array::FlexArray(__params);

                    let req = ::serde_json::to_value(__params)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;

                    let resp = self.transport
                        .call(&#method_enum.to_string(), req)
                        .await?;

                    let resp = ::serde_json::from_value(resp.result)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;
                    Ok(resp)
                }
            }
        }
    });

    // ---- Server arms ----
    let server_arms = methods.iter().map(|m| {
        let RpcMethod {
            name,
            method_enum,
            args,
            ..
        } = m;

        let arg_pats: Vec<_> = args.iter().map(|a| &a.pat).collect();
        let arg_tys: Vec<_> = args.iter().map(|a| &a.ty).collect();

        if arg_pats.is_empty() {
            quote! {
                #method_enum => {
                    // Accept null or [] (or ignore whatever) — just call.
                    let result = self.inner.#name().await?;
                    let result = ::serde_json::to_value(result)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;
                    Ok(result)
                }
            }
        } else {
            quote! {
                #method_enum => {
                    type __Params = ( #( #arg_tys, )* );
                    let ::wasmi_pdk::flex_array::FlexArray( ( #( #arg_pats, )* ) ): ::wasmi_pdk::flex_array::FlexArray<__Params> =
                        ::serde_json::from_value(params)
                            .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;


                    let result = self.inner.#name( #( #arg_pats ),* ).await?;
                    let result = ::serde_json::to_value(result)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;
                    Ok(result)
                }
            }
        }
    });

    quote! {
        #original

        pub struct #client_name<E: ::wasmi_pdk::api::ApiError> {
            transport: ::std::sync::Arc<dyn ::wasmi_pdk::transport::Transport<E> + Send + Sync>,
        }

        impl<E: ::wasmi_pdk::api::ApiError> #client_name<E> {
            pub fn new(transport: ::std::sync::Arc<dyn ::wasmi_pdk::transport::Transport<E> + Send + Sync>) -> Self {
                Self { transport }
            }
        }

        #[::async_trait::async_trait]
        impl<E: ::wasmi_pdk::api::ApiError> #trait_name for #client_name<E> {
            type Error = E;
            #(#client_methods)*
        }

        pub struct #server_name<T: #trait_name> {
            inner: ::std::sync::Arc<T>,
        }

        impl<T: #trait_name> #server_name<T> {
            pub fn new(inner: ::std::sync::Arc<T>) -> Self {
                Self { inner }
            }
        }

        #[::async_trait::async_trait]
        impl<T: #trait_name> ::wasmi_pdk::api::RequestHandler<T::Error> for #server_name<T> {
            async fn handle(&self, method: &str, params: ::serde_json::Value) -> Result<::serde_json::Value, T::Error> {
                let m = ::std::str::FromStr::from_str(method)
                    .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::MethodNotFound.into())?;
                match m {
                    #(#server_arms),*,
                    _ => Err(::wasmi_pdk::rpc_message::RpcErrorCode::MethodNotFound.into())
                }
            }
        }
    }
}
