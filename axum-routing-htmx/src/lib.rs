#![doc = include_str!("../../README.md")]
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
use dyn_fmt::AsStrFormatExt;

pub struct HtmxHandler<S = ()> {
    pub htmx_method: HtmxMethod,
    pub format_path: &'static str,
    pub axum_path: &'static str,
    pub method_router: MethodRouter<S>,
}

impl<S> HtmxHandler<S> {
    pub fn htmx_path<'a, T: Display + ?Sized + 'a>(
        &self,
        args: impl IntoIterator<Item = &'a T> + Clone,
    ) -> String {
        self.format_path.format(args)
    }
}

#[non_exhaustive]
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

type TypedHandler<S = ()> = fn() -> HtmxHandler<S>;
pub use axum_routing_htmx_macros::hx_delete;
pub use axum_routing_htmx_macros::hx_get;
pub use axum_routing_htmx_macros::hx_patch;
pub use axum_routing_htmx_macros::hx_post;
pub use axum_routing_htmx_macros::hx_put;

/// A trait that allows typed routes, created with the `hx_` macros to
/// be added to an axum router.
pub trait HtmxRouter: Sized {
    /// The state type of the router.
    type State: Clone + Send + Sync + 'static;

    /// Add an HTMX route to the router.
    ///
    /// Typed handlers are functions that return [`HtmxHandler`].
    fn htmx_route(self, handler: TypedHandler<Self::State>) -> Self;
}

impl<S> HtmxRouter for axum::Router<S>
where
    S: Send + Sync + Clone + 'static,
{
    type State = S;

    fn htmx_route(self, handler: TypedHandler<Self::State>) -> Self {
        let handler = handler();
        self.route(handler.axum_path, handler.method_router)
    }
}
