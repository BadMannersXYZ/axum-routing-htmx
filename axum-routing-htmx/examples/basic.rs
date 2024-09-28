#![allow(unused)]
use axum::extract::State;
use axum_routing_htmx::{hx_get, hx_post, HtmxRouter};

struct FooBar<S = ()> {
    pub htmx_method: ::axum_routing_htmx::HtmxMethod,
    pub axum_path: &'static str,
    pub method_router: ::axum::routing::MethodRouter<S>,
}

impl FooBar {
    fn htmx_path() -> String {
        format!("/title")
    }
}

impl<S> ::axum_routing_htmx::HtmxHandler<S> for FooBar<S> {
    fn axum_path(&self) -> &'static str {
        self.axum_path
    }
    fn method_router(&self) -> ::axum::routing::MethodRouter<S> {
        self.method_router.clone()
    }
}

#[hx_get("/title")]
async fn title_handler(State(state): State<String>) -> String {
    format!("Hello from {state}!")
}

#[hx_post("/button/:id")]
async fn button_handler(id: u32) -> String {
    format!("You clicked button #{id}!")
}

#[hx_get("/")]
async fn index_handler() -> String {
    let title = title_handler();
    let button = button_handler();
    format!(
        "<html>
            <head>
                <meta charset=\"utf-8\" />
                <script src=\"https://unpkg.com/htmx.org@2.0.2/dist/htmx.js\" crossorigin=\"anonymous\" />
            </head>
            <body>
                <h1 {}=\"{}\" hx-trigger=\"load\">Loading...</h1>
                <button {}=\"{}\" id=\"button-1\">Click me!</button>
            </body>
        </html>",
        title.htmx_method,
        title.htmx_path(),
        button.htmx_method,
        button.htmx_path(1),
    )
}

fn main() {
    let router: axum::Router = axum::Router::new()
        .htmx_route(title_handler())
        .with_state("axum-routing-htmx".to_string())
        .htmx_route(button_handler())
        .htmx_route(index_handler());
}
