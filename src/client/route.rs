use dioxus::prelude::*;
use crate::client::pages::{leaderboard::Leaderboard, login::Login, navbar::Navbar};

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[route("/login")]
    Login,

    #[layout(Navbar)]
    #[route("/leaderboard")]
    Leaderboard,
    // #[end_layout]
    // make page not found
}

impl Route {
    pub fn as_str(&self) -> &'static str {
        match self {
            Route::Leaderboard => "Classement",
            Route::Login => "Connexion",
        }
    }
}

pub fn app() -> Element {
    rsx! {
        document::Stylesheet { href : MAIN_CSS }
        Router::<Route> { }
    }
}
