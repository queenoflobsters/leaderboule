use dioxus::prelude::*;

mod pages;
mod components;

use pages::leaderboard::Leaderboard;
use pages::navbar::Navbar;

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(Navbar)]
        #[route("/leaderboard")]
        Leaderboard,
    // #[end_layout]
    // make page not found
}

impl Route {
    fn as_str(&self) -> &'static str {
        match self {
            Route::Leaderboard => "Classement",
        }
    }
}

pub fn app() -> Element {
    rsx! {
        document::Stylesheet { href : MAIN_CSS }
        Router::<Route> { }
    }
}
