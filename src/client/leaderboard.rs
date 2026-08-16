use crate::api::db::global::{self, LeaderboardUserCard};
use dioxus::prelude::*;

const LEADERBOARD_CSS: Asset = asset!("assets/leaderboard.css");
const RELOAD_SVG: Asset = asset!("assets/reload.svg");
const SORT_SVG: Asset = asset!("assets/sort.svg");
const TROPHY_SVG: Asset = asset!("assets/trophy.svg");
const BROKEN_HEART_SVG: Asset = asset!("assets/broken-heart.svg");
const TOTAL_SVG: Asset = asset!("assets/total.svg");
const PERCENT_SVG: Asset = asset!("assets/percent.svg");

#[component]
pub fn Leaderboard() -> Element {
    let mut refresh_count = use_signal(|| 0);
    let page = use_signal(|| 0);
    let page_size = use_signal(|| 5);

    rsx! {
        document::Stylesheet { href: LEADERBOARD_CSS }

        div { class: "leaderboard-container",

            Banner { on_reload: move |_| refresh_count += 1 }

            div { class: "cards",
                SuspenseBoundary {
                    fallback: |_| rsx! { p { class: "abnormal-state-message", "Patience..." } },
                    // 2. The suspended component receives clean signals
                    PerfCardsList { refresh_count, page, page_size }
                }
            }
        }
    }
}

#[component]
fn PerfCardsList(
    refresh_count: Signal<usize>,
    page: Signal<u64>,
    page_size: Signal<u64>,
) -> Element {
    let cards_resource = use_server_future(move || {
        let _ = refresh_count(); // Triggered on reload button click
        let p = page();
        let size = page_size();
        async move { global::get_leaderboard_cards(p, size).await }
    })?;

    match cards_resource() {
        Some(Ok(user_perfs)) => rsx! {
            for (i, user) in user_perfs.iter().enumerate() {
                CardItem { index: i, user: user.clone() }
            }
        },
        Some(Err(err)) => rsx! {
            p { class: "pending-messages", "Oulah... Erreur\n{err}" }
        },
        None => rsx! {
            p { class: "pending-messages", "Patience..." }
        },
    }
}

#[component]
fn Banner(on_reload: EventHandler<MouseEvent>) -> Element {
    rsx! {
        div { class: "banner",
            input {
                class: "search-input",
                r#type: "search",
                placeholder: "Search players...",
                // value: "{search_query}",
                // oninput: move |evt| {
                //     let value = evt.value();
                //     search_query.set(value.clone());
                //     if let Some(handler) = on_search {
                //         handler.call(value);
                //     }
                // },
            }

            button {
                class: "sort-button",
                onclick: move |evt| on_reload.call(evt),
                img { class: "banner-icon", src: SORT_SVG, alt: "Recharger", }
            }
            button {
                class: "reload-button",
                onclick: move |evt| on_reload.call(evt),
                img { class: "banner-icon", src: RELOAD_SVG, alt: "Recharger", }
            }
        }
    }
}

#[component]
fn CardItem(index: usize, user: LeaderboardUserCard) -> Element {
    let games_lost = user.games_played.saturating_sub(user.games_won);
    let ratio = (100 * user.games_won)
        .checked_div(user.games_played)
        .unwrap_or(0);
    let podium_class = match index {
        0 => "first",
        1 => "second",
        2 => "third",
        _ => "",
    };

    rsx! {
        div { class: "card {podium_class}",
            div { class: "player-username", "{user.username}" }
            div { class: "player-elo", "{user.elo}" }
            div { class: "player-stats",
                span { class: "stat-container games-won",
                    img { class: "stat-icon", src: TROPHY_SVG}
                    "{user.games_won}"
                }
                span { class: "stat-container games-lost",
                    img { class: "stat-icon", src: BROKEN_HEART_SVG}
                    "{games_lost}"
                }
                span { class: "stat-container games-played",
                    img { class: "stat-icon", src: TOTAL_SVG}
                    "{user.games_played}"
                }
                span { class: "stat-container ratio",
                    img { class: "stat-icon", src: PERCENT_SVG}
                    "{ratio}"
                }
            }
        }
    }
}
