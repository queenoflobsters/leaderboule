use dioxus::prelude::*;

use crate::api::db::current_user::{self, GameSearchItem};

const PAGE_SWITCHER_CSS: Asset = asset!("assets/style/page_switcher.css");

#[component]
pub fn History() -> Element {
    let mut current_page = use_signal(|| 0);

    rsx! {
        div { class: "history-container",

            SuspenseBoundary {
                fallback: |_| rsx! { p { class: "abnormal-state-message", "Patience..." } },
                GameList { current_page }

            }

            PageSwitcher {
                current_page: *current_page.read(),
                current_page_change: move |val| current_page.set(val)
            }
        }
    }
}

#[component]
fn GameList(current_page: ReadSignal<u64>) -> Element {
    let cards_resource = use_server_future(move || {
        let p = current_page();
        async move { current_user::get_game_history(p).await }
    })?;
    match cards_resource() {
        Some(Ok(game_card)) if game_card.is_empty() => rsx! {
            p { class: "abnormal-state-message", "Aucune partie jouée"}
        },
        Some(Ok(game_card)) => rsx! {
            for game in &game_card {{
                rsx! { GameCard { key: "{game.played_at}", game: game.clone() }}
            }}
        },
        Some(Err(e)) => rsx! {
            p { class: "abnormal-state-message", "Oulah... Erreur :\n{e}" }
        },
        None => rsx! {
            p { class: "abnormal-state-message", "Patience..." }
        },
    }
}

#[component]
fn GameCard(game: GameSearchItem) -> Element {
    rsx! {
        div {"{game.elo_change}"}
        div {"{game.won_score}"}
        div {"{game.lost_score}"}
        div {"{game.won_players:?}"}
        div {"{game.lost_players:?}"}
        div {"{game.played_at}"}
    }
}

#[component]
fn PageSwitcher(current_page: u64, current_page_change: EventHandler<u64>) -> Element {
    rsx! {
        document::Stylesheet { href: PAGE_SWITCHER_CSS }
        div { class: "page-switcher-container",
            button { class: "page-switcher-button",
                onclick: move |_| current_page_change.call(0),
                "<<<"
            }
            button { class: "page-switcher-button",
                onclick: move |_| if current_page > 0 { current_page_change.call(current_page -1 ) }, // WARN underflow problems ?
                "<<"
            }
            div { class: "page-switcher-text",
                "{current_page+1}"
            }
            button { class: "page-switcher-button",
                onclick: move |_| if current_page < 13 { current_page_change.call(current_page + 1) },
                ">>"
            }
        }
    }
}
