
mod api;
mod client;
#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
fn main() {
    use crate::server::auth;
    use dioxus::{logger::tracing};
    use dioxus::server::axum::{response::Html, routing::get, middleware};

    // Initialize the logger
    dioxus::logger::initialize_default();

    // Read the environment variables from the .env
    match dotenvy::dotenv() {
        Ok(_) => (),
        Err(e) => tracing::error!("Failed to read the .env : {}", e)
    }

    async fn serve_landing() -> Html<String> {
        let content = tokio::fs::read_to_string("public/landing.html")
            .await
            .unwrap_or_else(|_| String::from(
                "<h1>Oulah... Y'a un gros problème</h1><p>J'ai pas trouvé la page d'accueil frero</p>"
            ));
        Html(content)
    }

    dioxus::serve(|| async move {        
        let router = dioxus::server::router(client::route::app)
            .layer(middleware::from_fn(auth::server_auth_guard))
            .route("/", get(serve_landing));
        Ok(router)
    });
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(client::route::app);
}
