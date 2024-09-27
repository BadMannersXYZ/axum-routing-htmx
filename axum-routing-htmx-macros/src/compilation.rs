use quote::ToTokens;
use syn::{spanned::Spanned, PatType};

use self::parsing::PathParam;

use super::*;

pub struct CompiledRoute {
    #[allow(clippy::type_complexity)]
    pub path_params: Vec<(Slash, PathParam)>,
    pub query_params: Vec<(Ident, Box<Type>)>,
    pub state: Type,
    pub route_lit: LitStr,
}

impl CompiledRoute {
    pub fn to_axum_path_string(&self) -> String {
        let mut path = String::new();

        for (_slash, param) in &self.path_params {
            path.push('/');
            match param {
                PathParam::Capture(lit, _, _, _) => {
                    path.push(':');
                    path.push_str(&lit.value())
                }
                PathParam::WildCard(lit, _, _, _) => {
                    path.push('*');
                    path.push_str(&lit.value());
                }
                PathParam::Static(lit) => path.push_str(&lit.value()),
            }
            // if colon.is_some() {
            //     path.push(':');
            // }
            // path.push_str(&ident.value());
        }

        path
    }

    pub fn to_format_path_string(&self, include_query_params: bool) -> String {
        let mut path = String::new();

        for (_slash, param) in &self.path_params {
            path.push('/');
            match param {
                PathParam::Capture(_, _, _, _) => {
                    path.push_str("{}");
                }
                PathParam::WildCard(_, _, _, _) => {
                    path.push_str("{}");
                }
                PathParam::Static(lit) => path.push_str(&lit.value()),
            }
        }

        if include_query_params {
            for (i, param) in self.query_params.iter().enumerate() {
                if i == 0 {
                    path.push('?');
                } else {
                    path.push('&');
                }
                path.push_str(&param.0.to_string());
                path.push_str("={}");
            }
        }

        path
    }

    /// Removes the arguments in `route` from `args`, and merges them in the output.
    pub fn from_route(mut route: Route, function: &ItemFn) -> syn::Result<Self> {
        let sig = &function.sig;
        let mut arg_map = sig
            .inputs
            .iter()
            .filter_map(|item| match item {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(pat_type) => Some(pat_type),
            })
            .filter_map(|pat_type| match &*pat_type.pat {
                syn::Pat::Ident(ident) => Some((ident.ident.clone(), pat_type.ty.clone())),
                _ => None,
            })
            .collect::<HashMap<_, _>>();

        for (_slash, path_param) in &mut route.path_params {
            match path_param {
                PathParam::Capture(_lit, _colon, ident, ty) => {
                    let (new_ident, new_ty) = arg_map.remove_entry(ident).ok_or_else(|| {
                        syn::Error::new(
                            ident.span(),
                            format!("path parameter `{}` not found in function arguments", ident),
                        )
                    })?;
                    *ident = new_ident;
                    *ty = new_ty;
                }
                PathParam::WildCard(_lit, _star, ident, ty) => {
                    let (new_ident, new_ty) = arg_map.remove_entry(ident).ok_or_else(|| {
                        syn::Error::new(
                            ident.span(),
                            format!("path parameter `{}` not found in function arguments", ident),
                        )
                    })?;
                    *ident = new_ident;
                    *ty = new_ty;
                }
                PathParam::Static(_lit) => {}
            }
        }

        let mut query_params = Vec::new();
        for ident in route.query_params {
            let (ident, ty) = arg_map.remove_entry(&ident).ok_or_else(|| {
                syn::Error::new(
                    ident.span(),
                    format!(
                        "query parameter `{}` not found in function arguments",
                        ident
                    ),
                )
            })?;
            query_params.push((ident, ty));
        }

        Ok(Self {
            route_lit: route.route_lit,
            path_params: route.path_params,
            query_params,
            state: route.state.unwrap_or_else(|| guess_state_type(sig)),
        })
    }

    pub fn path_extractor(&self) -> Option<TokenStream2> {
        if !self.path_params.iter().any(|(_, param)| param.captures()) {
            return None;
        }

        let path_iter = self
            .path_params
            .iter()
            .filter_map(|(_slash, path_param)| path_param.capture());
        let idents = path_iter.clone().map(|item| item.0);
        let types = path_iter.clone().map(|item| item.1);
        Some(quote! {
            ::axum::extract::Path((#(#idents,)*)): ::axum::extract::Path<(#(#types,)*)>,
        })
    }

    pub fn query_extractor(&self) -> Option<TokenStream2> {
        if self.query_params.is_empty() {
            return None;
        }

        let idents = self.query_params.iter().map(|item| &item.0);
        Some(quote! {
            ::axum::extract::Query(__QueryParams__ {
                #(#idents,)*
            }): ::axum::extract::Query<__QueryParams__>,
        })
    }

    pub fn query_params_struct(&self) -> Option<TokenStream2> {
        match self.query_params.is_empty() {
            true => None,
            false => {
                let idents = self.query_params.iter().map(|item| &item.0);
                let types = self.query_params.iter().map(|item| &item.1);
                let derive = quote! { #[derive(::serde::Deserialize)] };
                Some(quote! {
                    #derive
                    struct __QueryParams__ {
                        #(#idents: #types,)*
                    }
                })
            }
        }
    }

    pub fn extracted_idents(&self) -> Vec<Ident> {
        let mut idents = Vec::new();
        for (_slash, path_param) in &self.path_params {
            if let Some((ident, _ty)) = path_param.capture() {
                idents.push(ident.clone());
            }
            // if let Some((_colon, ident, _ty)) = colon {
            //     idents.push(ident.clone());
            // }
        }
        for (ident, _ty) in &self.query_params {
            idents.push(ident.clone());
        }
        idents
    }

    /// The arguments not used in the route.
    /// Map the identifier to `_arg_{i}: Type`.
    pub fn remaining_pattypes_numbered(
        &self,
        args: &Punctuated<FnArg, Comma>,
    ) -> Punctuated<PatType, Comma> {
        args.iter()
            .enumerate()
            .filter_map(|(i, item)| {
                if let FnArg::Typed(pat_type) = item {
                    if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                        if self.path_params.iter().any(|(_slash, path_param)| {
                            if let Some((path_ident, _ty)) = path_param.capture() {
                                path_ident == &pat_ident.ident
                            } else {
                                false
                            }
                        }) || self
                            .query_params
                            .iter()
                            .any(|(query_ident, _)| query_ident == &pat_ident.ident)
                        {
                            return None;
                        }
                    }

                    let mut new_pat_type = pat_type.clone();
                    let ident = format_ident!("_arg_{}", i);
                    new_pat_type.pat = Box::new(parse_quote!(#ident));
                    Some(new_pat_type)
                } else {
                    unimplemented!("Self type is not supported")
                }
            })
            .collect()
    }

    pub(crate) fn to_doc_comments(&self) -> TokenStream2 {
        let doc = format!(
            "# Handler information
- Path: `{}`
- State: `{}`",
            self.route_lit.value(),
            self.state.to_token_stream(),
        );

        quote!(
            #[doc = #doc]
        )
    }
}

fn guess_state_type(sig: &syn::Signature) -> Type {
    for arg in &sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            // Returns `T` if the type of the last segment is exactly `State<T>`.
            if let Type::Path(ty) = &*pat_type.ty {
                let last_segment = ty.path.segments.last().unwrap();
                if last_segment.ident == "State" {
                    if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                        if args.args.len() == 1 {
                            if let GenericArgument::Type(ty) = args.args.first().unwrap() {
                                return ty.clone();
                            }
                        }
                    }
                }
            }
        }
    }

    parse_quote! { () }
}
