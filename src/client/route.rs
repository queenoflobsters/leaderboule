use crate::client::pages::{
    account::Account, leaderboard::Leaderboard, login::Login, logout::Logout, navbar::Navbar, page_not_found::PageNotFound
};
use dioxus::prelude::*;

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[route("/login")]
    Login,

    #[layout(Navbar)]
    #[route("/leaderboard")]
    Leaderboard,
    #[route("/logout")]
    Logout,
    #[route("/account")]
    Account, 
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> }

     // TODO make page not found
}

impl Route {
    pub fn as_str(&self) -> &'static str {
        match self {
            Route::Leaderboard => "Classement",
            Route::Login => "Connexion",
            Route::Logout => "Déconnexion",
            Route::Account => "Mon Compte",
            Route::PageNotFound { .. } => "Page non trouvée"
        }
    }

    pub fn is_public(&self) -> bool {
        match self {
            Route::Login => true,
            _ => false,
        }
    }
}

pub fn app() -> Element {
    rsx! {
        document::Stylesheet { href : MAIN_CSS }
        Router::<Route> { }
    }
}
