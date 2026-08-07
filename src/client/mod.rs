use dioxus::prelude::*;

mod pages;
mod components;

use pages::leaderboard::Leaderboard;

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[route("/leaderboard")]
    Leaderboard,
}

pub fn app() -> Element {
    rsx! {
        document::Stylesheet { href : MAIN_CSS }
        Router::<Route> { }
    }
}
