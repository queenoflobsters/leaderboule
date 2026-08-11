use dioxus::prelude::*;

mod api;
mod client;
#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
fn main() {
    use crate::server::auth;
    use dioxus::server::axum::middleware;
    use dioxus_server::axum::{response::Html, routing::get};

    async fn serve_landing() -> Html<String> {
        let content = tokio::fs::read_to_string("public/landing.html")
            .await
            .unwrap_or_else(|_| String::from(
                "<h1>Oulah... Y'a un gros problème</h1><p>J'ai pas trouvé la page d'accueil frero</p>"
            ));
        Html(content)
    }

    dioxus::logger::initialize_default();

    dioxus::serve(|| async move {
        let router = dioxus::server::router(client::route::app)
            // .layer(...)
            .layer(middleware::from_fn(auth::middleware))
            .route("/", get(serve_landing));
        Ok(router)
    });
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(client::route::app);
}
