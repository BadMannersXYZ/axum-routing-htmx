use quote::ToTokens;
use syn::{token::Star, LitInt};

use super::*;

struct RouteParser {
    path_params: Vec<(Slash, PathParam)>,
    query_params: Vec<Ident>,
}

impl RouteParser {
    fn new(lit: LitStr) -> syn::Result<Self> {
        let val = lit.value();
        let span = lit.span();
        let split_route = val.split('?').collect::<Vec<_>>();
        if split_route.len() > 2 {
            return Err(syn::Error::new(span, "expected at most one '?'"));
        }

        let path = split_route[0];
        if !path.starts_with('/') {
            return Err(syn::Error::new(span, "expected path to start with '/'"));
        }
        let path = path.strip_prefix('/').unwrap();

        let mut path_params = Vec::new();
        #[allow(clippy::never_loop)]
        for path_param in path.split('/') {
            path_params.push((
                Slash(span),
                PathParam::new(path_param, span, Box::new(parse_quote!(()))),
            ));
        }

        let path_param_len = path_params.len();
        for (i, (_slash, path_param)) in path_params.iter().enumerate() {
            match path_param {
                PathParam::WildCard(_, _, _, _) => {
                    if i != path_param_len - 1 {
                        return Err(syn::Error::new(
                            span,
                            "wildcard path param must be the last path param",
                        ));
                    }
                }
                PathParam::Capture(_, _, _, _) => (),
                PathParam::Static(lit) => {
                    if lit.value() == "*" && i != path_param_len - 1 {
                        return Err(syn::Error::new(
                            span,
                            "wildcard path param must be the last path param",
                        ));
                    }
                }
            }
        }

        let mut query_params = Vec::new();
        if split_route.len() == 2 {
            let query = split_route[1];
            for query_param in query.split('&') {
                query_params.push(Ident::new(query_param, span));
            }
        }

        Ok(Self {
            path_params,
            query_params,
        })
    }
}

pub enum PathParam {
    WildCard(LitStr, Star, Ident, Box<Type>),
    Capture(LitStr, Colon, Ident, Box<Type>),
    Static(LitStr),
}

impl PathParam {
    pub fn captures(&self) -> bool {
        matches!(self, Self::Capture(..) | Self::WildCard(..))
    }

    // pub fn lit(&self) -> &LitStr {
    //     match self {
    //         Self::Capture(lit, _, _, _) => lit,
    //         Self::WildCard(lit, _, _, _) => lit,
    //         Self::Static(lit) => lit,
    //     }
    // }

    pub fn capture(&self) -> Option<(&Ident, &Type)> {
        match self {
            Self::Capture(_, _, ident, ty) => Some((ident, ty)),
            Self::WildCard(_, _, ident, ty) => Some((ident, ty)),
            _ => None,
        }
    }

    fn new(str: &str, span: Span, ty: Box<Type>) -> Self {
        if str.starts_with(':') {
            let str = str.strip_prefix(':').unwrap();
            Self::Capture(
                LitStr::new(str, span),
                Colon(span),
                Ident::new(str, span),
                ty,
            )
        } else if str.starts_with('*') && str.len() > 1 {
            let str = str.strip_prefix('*').unwrap();
            Self::WildCard(
                LitStr::new(str, span),
                Star(span),
                Ident::new(str, span),
                ty,
            )
        } else {
            Self::Static(LitStr::new(str, span))
        }
    }
}

pub struct Security(pub Vec<(LitStr, StrArray)>);
impl Parse for Security {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner;
        braced!(inner in input);

        let mut arr = Vec::new();
        while !inner.is_empty() {
            let scheme = inner.parse::<LitStr>()?;
            let _ = inner.parse::<Token![:]>()?;
            let scopes = inner.parse::<StrArray>()?;
            let _ = inner.parse::<Token![,]>().ok();
            arr.push((scheme, scopes));
        }

        Ok(Self(arr))
    }
}

impl ToString for Security {
    fn to_string(&self) -> String {
        let mut s = String::new();
        s.push('{');
        for (i, (scheme, scopes)) in self.0.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&scheme.value());
            s.push_str(": ");
            s.push_str(&scopes.to_string());
        }
        s.push('}');
        s
    }
}

pub struct Responses(pub Vec<(LitInt, Type)>);
impl Parse for Responses {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner;
        braced!(inner in input);

        let mut arr = Vec::new();
        while !inner.is_empty() {
            let status = inner.parse::<LitInt>()?;
            let _ = inner.parse::<Token![:]>()?;
            let ty = inner.parse::<Type>()?;
            let _ = inner.parse::<Token![,]>().ok();
            arr.push((status, ty));
        }

        Ok(Self(arr))
    }
}

impl ToString for Responses {
    fn to_string(&self) -> String {
        let mut s = String::new();
        s.push('{');
        for (i, (status, ty)) in self.0.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&status.to_string());
            s.push_str(": ");
            s.push_str(&ty.to_token_stream().to_string());
        }
        s.push('}');
        s
    }
}

#[derive(Clone)]
pub struct StrArray(pub Vec<LitStr>);
impl Parse for StrArray {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner;
        bracketed!(inner in input);
        let mut arr = Vec::new();
        while !inner.is_empty() {
            arr.push(inner.parse::<LitStr>()?);
            inner.parse::<Token![,]>().ok();
        }
        Ok(Self(arr))
    }
}

impl ToString for StrArray {
    fn to_string(&self) -> String {
        let mut s = String::new();
        s.push('[');
        for (i, lit) in self.0.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push('"');
            s.push_str(&lit.value());
            s.push('"');
        }
        s.push(']');
        s
    }
}

pub struct Route {
    pub path_params: Vec<(Slash, PathParam)>,
    pub query_params: Vec<Ident>,
    pub state: Option<Type>,
    pub route_lit: LitStr,
}

impl Parse for Route {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let route_lit = input.parse::<LitStr>()?;
        let route_parser = RouteParser::new(route_lit.clone())?;
        let state = match input.parse::<kw::with>() {
            Ok(_) => Some(input.parse::<Type>()?),
            Err(_) => None,
        };

        Ok(Route {
            path_params: route_parser.path_params,
            query_params: route_parser.query_params,
            state,
            route_lit,
        })
    }
}

mod kw {
    syn::custom_keyword!(with);
}
