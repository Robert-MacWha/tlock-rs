use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

/// Representation of a single RPC method extracted from a trait.
struct RpcMethod {
    name: syn::Ident,          // e.g. "tlock_ping"
    method_enum: syn::Path,    // e.g. Methods::TlockPing
    arg: Option<syn::PatType>, // zero or one argument
    output: syn::Type,         // Result<Ret, Error>
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
            let arg = match inputs.next() {
                None => None,
                Some(syn::FnArg::Typed(pat)) => Some((pat).clone()),
                Some(other) => return Err(syn::Error::new_spanned(other, "unsupported arg")),
            };
            if inputs.next().is_some() {
                return Err(syn::Error::new_spanned(
                    &sig.ident,
                    "only 0 or 1 arg allowed",
                ));
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
                arg,
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

    // Client impl
    let client_methods = methods.iter().map(|m| {
        let RpcMethod {
            name,
            method_enum,
            arg,
            output,
        } = m;
        if let Some(arg) = arg {
            let pat = &arg.pat;
            quote! {
                async fn #name(&self, #arg) -> #output {
                    let req = ::serde_json::to_value(#pat)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;
                    let resp = self.transport
                        .call(&#method_enum.to_string(), req)
                        .await?;
                    let resp = ::serde_json::from_value(resp.result)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;
                    Ok(resp)
                }
            }
        } else {
            quote! {
                async fn #name(&self) -> #output {
                    let req = ::serde_json::Value::Null;
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

    // Server arms
    let server_arms = methods.iter().map(|m| {
        let RpcMethod {
            name,
            method_enum,
            arg,
            ..
        } = m;
        if let Some(arg) = arg {
            let pat = &arg.pat;
            let ty = &arg.ty;
            quote! {
                #method_enum => {
                    let #pat: #ty = ::serde_json::from_value(params)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;
                    let result = self.inner.#name(#pat).await?;
                    let result = ::serde_json::to_value(result)
                        .map_err(|_| ::wasmi_pdk::rpc_message::RpcErrorCode::ParseError.into())?;
                    Ok(result)
                }
            }
        } else {
            quote! {
                #method_enum => {
                    let result = self.inner.#name().await?;
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
