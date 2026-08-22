use crate::client::{
    account::Account, leaderboard::Leaderboard, login::Login, logout::Logout, navbar::Navbar,
    page_not_found::PageNotFound, new_game::NewGame, history::History,
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
    #[route("/newgame")]
    NewGame,
    #[route("/history")]
    History
    // TODO make page not found
}

impl Route {
    pub fn title(&self) -> &'static str {
        match self {
            Route::Leaderboard => "Classement",
            Route::Login => "Connexion",
            Route::Logout => "Déconnexion",
            Route::Account => "Mon Compte",
            Route::PageNotFound { .. } => "Page non trouvée",
            Route::NewGame => "Nouvelle Partie",
            Route::History => "Historique",
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
