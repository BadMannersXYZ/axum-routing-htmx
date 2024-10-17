//!
//! ## Basic usage
//! The following example demonstrates the basic usage of the library.
//! On top of any regular handler, you can add the [`route`] macro to create a typed route.
//! Any path- or query-parameters in the url will be type-checked at compile-time, and properly
//! extracted into the handler.
//!
//! ```
#![doc = include_str!("../examples/basic.rs")]
//! ```
//!
//! Some valid url's as get-methods are:
//! - `/item/1?amount=2&offset=3`
//! - `/item/1?amount=2`
//! - `/item/1?offset=3`
//! - `/item/500`
//!

use std::fmt::Display;

use axum::routing::MethodRouter;

/// The trait expected by the Router to add HTMX routes.
pub trait HtmxHandler<S> {
    fn axum_router(self) -> (&'static str, MethodRouter<S>);
}

#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
/// The HTTP verbs supported by HTMX.
pub enum HtmxMethod {
    Get,
    Post,
    Delete,
    Patch,
    Put,
}

impl Display for HtmxMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            HtmxMethod::Get => "hx-get",
            HtmxMethod::Post => "hx-post",
            HtmxMethod::Delete => "hx-delete",
            HtmxMethod::Patch => "hx-patch",
            HtmxMethod::Put => "hx-put",
        })
    }
}

pub use axum_routing_htmx_macros::{hx_delete, hx_get, hx_patch, hx_post, hx_put};

/// A trait that allows typed routes, created with the `hx_` macros to
/// be added to an axum router.
pub trait HtmxRouter: Sized {
    /// The state type of the router.
    type State: Send + Sync + Clone + 'static;

    /// Add an HTMX route to the router.
    ///
    /// Typed handlers are functions that return [`HtmxHandler`].
    fn htmx_route(self, handler: impl HtmxHandler<Self::State>) -> Self;
}

impl<S> HtmxRouter for axum::Router<S>
where
    S: Send + Sync + Clone + 'static,
{
    type State = S;

    fn htmx_route(self, handler: impl HtmxHandler<Self::State>) -> Self {
        let (path, method_router) = handler.axum_router();
        self.route(path, method_router)
    }
}
