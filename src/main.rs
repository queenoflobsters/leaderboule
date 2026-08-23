/// ENTRYPOINT
///
/// Bienvenue sur le code de Leaderboule,
/// l'application de de classement et d'élo du club de pétanque de l'INSA de Lyon
/// Les variables et le code sont écrits en anglais par convention et habitude mais
/// je tente de faire des commentaires en français.
/// Désolé pour les fautes je suis pas particulièrement concentré
///
/// Écrit et maintenu par Gabriel Gonguet,
/// Secrétaire général du comité centrale du partie de la pétanque 


/// Module pour le code du serveur,
/// maintenu derrière un config guard
#[cfg(feature = "server")]
mod server;

/// Module pour l'intéraction entre le serveur et le client
mod api;

/// Module contenant le code de l'application client
mod client;

/// Entry point du serveur
#[cfg(feature = "server")]
fn main() {
    use crate::server::auth;
    use dioxus::{logger::tracing};
    use dioxus::server::axum::{response::Html, routing::get, middleware};

    // Initialise le logger
    dioxus::logger::initialize_default();

    // Lis les variables d'environnement du .env
    match dotenvy::dotenv() {
        Ok(_) => (),
        Err(e) => tracing::error!("Failed to read the .env : {}", e)
    }

    // Route servant un page HTML static pour l'accueil
    async fn serve_landing() -> Html<String> {
        let content = tokio::fs::read_to_string("public/landing.html")
            .await
            .unwrap_or_else(|_| String::from(
                "<h1>Oulah... Y'a un gros problème</h1><p>J'ai pas trouvé la page d'accueil frero</p>"
            ));
        Html(content)
    }

    // Création et lancement du runtime du serveur
    dioxus::serve(|| async move {        
        let router = dioxus::server::router(client::route::app)
            .layer(middleware::from_fn(auth::middleware::server_auth_guard))
            .route("/", get(serve_landing));
        Ok(router)
    });
}

/// Entrypoint de l'application
#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(client::route::app);
}
