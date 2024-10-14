use compilation::CompiledRoute;
use parsing::Route;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use std::collections::HashMap;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Colon, Comma, Slash},
    FnArg, GenericArgument, ItemFn, LitStr, PathArguments, Type,
};
#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

mod compilation;
mod parsing;

macro_rules! hx_route {
    ($method:ident, $enum_verb:literal, $axum_method:literal) => {
        #[doc = concat!("
A macro that generates HTMX-compatible statically-typed ", $enum_verb, " routes for axum handlers.

# Syntax
```ignore
#[", stringify!($method), "(\"<PATH>\" [with <STATE>])]
```
- `PATH` is the path of the route, with optional path parameters and query parameters,
    e.g. `/item/:id?amount&offset`.
- `STATE` is the type of axum-state, passed to the handler. This is optional, and if not
    specified, the state type is guessed based on the parameters of the handler.

# Example
```
use axum::extract::{State, Json};
use axum_routing_htmx::", stringify!($method), ";

#[", stringify!($method), "(\"/item/:id?amount&offset\")]
async fn item_handler(
    id: u32,
    amount: Option<u32>,
    offset: Option<u32>,
    State(state): State<String>,
    Json(json): Json<u32>,
) -> String {
    todo!(\"handle request\")
}
```

# State type
Normally, the state-type is guessed based on the parameters of the function:
If the function has a parameter of type `[..]::State<T>`, then `T` is used as the state type.
This should work for most cases, however when not sufficient, the state type can be specified
explicitly using the `with` keyword:
```ignore
#[", stringify!($method), "(\"/item/:id?amount&offset\" with String)]
```

# Internals
The macro expands to a function that returns an [`HtmxHandler<S>`].")]
        #[proc_macro_attribute]
        pub fn $method(attr: TokenStream, mut item: TokenStream) -> TokenStream {
            match _route(attr, item.clone(), $enum_verb, $axum_method) {
                Ok(tokens) => tokens.into(),
                Err(err) => {
                    let err: TokenStream = err.to_compile_error().into();
                    item.extend(err);
                    item
                }
            }
        }
    };
}

hx_route!(hx_get, "Get", "get");
hx_route!(hx_post, "Post", "post");
hx_route!(hx_delete, "Delete", "delete");
hx_route!(hx_patch, "Patch", "patch");
hx_route!(hx_put, "Put", "put");

fn _route(
    attr: TokenStream,
    item: TokenStream,
    enum_verb: &'static str,
    axum_method: &'static str,
) -> syn::Result<TokenStream2> {
    // Parse the route and function
    let route = syn::parse::<Route>(attr)?;
    let function = syn::parse::<ItemFn>(item)?;

    // Now we can compile the route
    let route = CompiledRoute::from_route(route, &function)?;
    let path_extractor = route.path_extractor();
    let query_extractor = route.query_extractor();
    let query_params_struct = route.query_params_struct();
    let state_type = &route.state;
    let axum_path = route.to_axum_path_string();
    let format_path = route.to_format_path_string();
    let remaining_numbered_pats = route.remaining_pattypes_numbered(&function.sig.inputs);
    let extracted_idents = route.extracted_idents();
    let remaining_numbered_idents = remaining_numbered_pats.iter().map(|pat_type| &pat_type.pat);
    let route_docs = route.to_doc_comments();

    // Get the variables we need for code generation
    let fn_name = &function.sig.ident;
    let fn_output = &function.sig.output;
    let vis = &function.vis;
    let asyncness = &function.sig.asyncness;
    let (impl_generics, ty_generics, where_clause) = &function.sig.generics.split_for_impl();
    let ty_generics = ty_generics.as_turbofish();
    let fn_docs = function
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"));
    let enum_method = format_ident!("{}", enum_verb);
    let http_method = format_ident!("{}", axum_method);
    let htmx_struct = format_ident!("__HtmxHandler_{}", fn_name);

    // Generate the code
    Ok(quote! {
        #[allow(non_camel_case_types)]
        #vis struct #htmx_struct<S> {
            /// Which HTMX method this corresponds with. The `Display` interface
            /// can be used to generate the HTML attribute name.
            pub htmx_method: ::axum_routing_htmx::HtmxMethod,
            /// The MethodRouter that must be consumed by axum.
            pub method_router: ::axum::routing::MethodRouter<S>,
        }

        impl<S> #htmx_struct<S> {
            /// Generates a path according to the expected fields of the handler.
            fn htmx_path(
                &self,
                #(#extracted_idents: impl ::std::fmt::Display,)*
            ) -> String {
                format!(#format_path, #(#extracted_idents,)*)
            }
        }

        impl<S> ::axum_routing_htmx::HtmxHandler<S> for #htmx_struct<S> {
            fn axum_router(self) -> (&'static str, ::axum::routing::MethodRouter<S>) {
                (#axum_path, self.method_router)
            }
        }

        #(#fn_docs)*
        #route_docs
        #vis fn #fn_name #impl_generics() -> #htmx_struct<#state_type> #where_clause {

            #query_params_struct

            #asyncness fn __inner #impl_generics(
                #path_extractor
                #query_extractor
                #remaining_numbered_pats
            ) #fn_output #where_clause {
                #function

                #fn_name #ty_generics(#(#extracted_idents,)* #(#remaining_numbered_idents,)* ).await
            }

            #htmx_struct {
                htmx_method: ::axum_routing_htmx::HtmxMethod::#enum_method,
                method_router: ::axum::routing::#http_method(__inner #ty_generics)
            }
        }
    })
}
