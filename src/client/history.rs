use dioxus::prelude::*;

use crate::api::{
    db::current_user::{self, GameSearchItem},
    utils,
};

const HISTORY_CSS: Asset = asset!("assets/style/history.css");
const PAGE_SWITCHER_CSS: Asset = asset!("assets/style/page_switcher.css");
const ARROW_UP_SVG: Asset = asset!("assets/icons/arrow-up.svg");
const ARROW_DOWN_SVG: Asset = asset!("assets/icons/arrow-down.svg");

#[component]
pub fn History() -> Element {
    let mut current_page = use_signal(|| 0);

    rsx! {
        document::Stylesheet { href: HISTORY_CSS }
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
    let played_at_str = utils::format_date_and_hour(game.played_at);
    let self_elo_change_class = if game.elo_change > 0 { "elo-gain" } else { "elo-loss" };
    let self_elo_change_icon = if game.elo_change > 0 { &ARROW_UP_SVG } else { &ARROW_DOWN_SVG };

    rsx! {
        div { class: "game-card",
            div { class: "game-card-banner",
                div { class: "game-card-elo-change {self_elo_change_class}",
                    img { class: "game-card-elo-change-icon", src: *self_elo_change_icon}
                    "{game.elo_change}"
                }
                div { class: "game-card-score",
                    "{game.won_score}  -  {game.lost_score}"
                }
                div { class: "game-card-time",
                    "{played_at_str}"
                }
            }
            div { class: "game-card-players-list",
                for p in &game.won_players {
                    {rsx! {
                        // div { class: "players-entry won",
                        div { class: "players-entry",
                            div { "{p.username}" }
                            div { class: "players-elo-change elo-gain",
                                img { class: "players-elo-change-icons", src: ARROW_UP_SVG}
                                "{p.elo}"
                            }
                        }
                    }}
                }
                hr { class: "players-list-separator" }
                for p in &game.lost_players {
                    {rsx! {
                        // div { class: "players-entry lost",
                        div { class: "players-entry",
                            div {  "{p.username}" }
                            div { class: "players-elo-change elo-loss",
                                img { class: "players-elo-change-icons", src: ARROW_DOWN_SVG}
                                "{p.elo}"
                            }
                        }
                    }}
                }
            }
        }
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
                onclick: move |_| if current_page > 0 { current_page_change.call(current_page -1 ) },                 "<<"
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
