use dioxus::prelude::*;

mod api;
mod client;
#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
fn main() {
    use dioxus_server::axum::{response::Html, routing::get};
    use dioxus::server::axum::middleware;

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

        let router = dioxus::server::router(client::app)
            // .layer(...)
            .layer(middleware::from_fn(server::auth_middleware))
            .route("/", get(serve_landing));
        Ok(router)
    });
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(client::app);
}
// use dioxus::prelude::*;

// mod api;
// mod client;
// #[cfg(feature = "server")]
// mod server;

// #[cfg(feature = "server")]
// fn main() {
//     use dioxus_server::axum::{middleware, response::Html, routing::get};

//     async fn serve_landing() -> Html<String> {
//         let content = tokio::fs::read_to_string("public/landing.html")
//             .await
//             .unwrap_or_else(|_| String::from(
//                 "<h1>Oulah... Y'a un gros problème</h1><p>J'ai pas trouvé la page d'accueil frero</p>"
//             ));
//         Html(content)
//     }

//     dioxus::logger::initialize_default();

//     dioxus::serve(|| async move {
//         let _ = server::get_db().await;

//         let router = dioxus::server::router(client::app)
            // .layer(middleware::from_fn(server::auth_middleware))
//             .route("/", get(serve_landing));
//         Ok(router)
//     });
// }

// #[cfg(not(feature = "server"))]
// fn main() {
//     dioxus::launch(client::app);
// }
