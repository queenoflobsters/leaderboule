use dioxus::prelude::*;
use crate::client::pages::{leaderboard::Leaderboard, login::Login, navbar::Navbar, logout::Logout, account::Account};

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
    Account
    // #[end_layout]
    // TODO make page not found
}

impl Route {
    pub fn as_str(&self) -> &'static str {
        match self {
            Route::Leaderboard => "Classement",
            Route::Login => "Connexion",
            Route::Logout => "Déconnexion",
            Route::Account => "Compte",
        }
    }
}

pub fn app() -> Element {
    rsx! {
        document::Stylesheet { href : MAIN_CSS }
        Router::<Route> { }
    }
}
