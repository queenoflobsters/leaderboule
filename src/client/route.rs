use crate::client::{
    account::Account, leaderboard::Leaderboard, login::Login, logout::Logout, navbar::Navbar,
    page_not_found::PageNotFound,
};
use dioxus::prelude::*;

const MAIN_CSS: Asset = asset!("/assets/style/main.css");

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[route("/login")]
    Login,
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },

    #[layout(Navbar)]
    #[route("/leaderboard")]
    Leaderboard,
    #[route("/logout")]
    Logout,
    #[route("/account")]
    Account,
    // TODO make page not found
}

impl Route {
    pub fn as_str(&self) -> &'static str {
        match self {
            Route::Leaderboard => "Classement",
            Route::Login => "Connexion",
            Route::Logout => "Déconnexion",
            Route::Account => "Mon Compte",
            Route::PageNotFound { .. } => "Page non trouvée",
        }
    }

    pub fn is_public(&self) -> bool {
        // I know about matches! I just don't like it
        match self {
            Route::Login => true,
            Route::PageNotFound { .. } => true,
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
