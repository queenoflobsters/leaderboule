use crate::client::{
    account::Account,
    history::History,
    leaderboard::Leaderboard,
    login::Login,
    logout::Logout,
    moderation::{
        Moderation, ModerationGamePage as ModerationGame, ModerationGames, ModerationUser,
    },
    navbar::Navbar,
    new_game::NewGame,
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
    #[route("/newgame")]
    NewGame,
    #[route("/history")]
    History, // TODO make page not found
    #[route("/moderation")]
    Moderation,
    #[route("/moderation/users/:user_id")]
    ModerationUser { user_id: String },
    #[route("/moderation/games")]
    ModerationGames,
    #[route("/moderation/games/:game_id")]
    ModerationGame { game_id: String },
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
            Route::Moderation => "Modération",
            Route::ModerationUser { .. } => "Inspecteur utilisateur",
            Route::ModerationGames => "Parties",
            Route::ModerationGame { .. } => "Édition de partie",
        }
    }

    #[cfg(feature = "server")]
    #[allow(clippy::match_like_matches_macro)] // Je n'aime pas matches!
    pub fn is_public(&self) -> bool {
        // I know about matches! I just don't like it
        match self {
            Route::Login => true,
            Route::PageNotFound { .. } => true,
            _ => false,
        }
    }

    #[cfg(feature = "server")]
    #[allow(clippy::match_like_matches_macro)] // Cohérent avec `is_public`
    pub fn is_admin_only(&self) -> bool {
        matches!(
            self,
            Route::Moderation
                | Route::ModerationUser { .. }
                | Route::ModerationGames
                | Route::ModerationGame { .. }
        )
    }
}

pub fn app() -> Element {
    rsx! {
        document::Stylesheet { href : MAIN_CSS }
        Router::<Route> { }
    }
}
