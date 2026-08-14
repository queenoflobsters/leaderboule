use dioxus::{prelude::*, CapturedError};

use crate::api::{get_users_performances, UserPerformance};

const LEADERBOARD_CSS: Asset = asset!("assets/leaderboard.css");
const RELOAD_SVG: Asset = asset!("assets/reload.svg");

#[component]
pub fn Leaderboard() -> Element {
    let user_perfs_hook = use_server_future(get_users_performances);
    let mut reload_hook = user_perfs_hook.clone();
    rsx! {
        document::Stylesheet { href : LEADERBOARD_CSS }

        div {
            class: "leaderboard-container",
            div {
                class: "banner",
                Banner { on_click: move |_| {
                        if let Ok(resource) = &mut reload_hook {
                            resource.restart();
                        }
                    } }
            }
            div {
                class: "cards",
                SuspenseBoundary {
                    fallback: move |_| rsx! { p { class:"abnormal-state-message", "Patience..." } },
                    PerfCardsManager { user_perfs_hook }
                }
            }
        }

    }
}

#[component]
fn PerfCardsManager(
    user_perfs_hook: Result<Resource<Result<Vec<UserPerformance>, CapturedError>>, RenderError>,
) -> Element {
    match user_perfs_hook?() {
        Some(Ok(user_perfs)) => rsx! {PerfCards { user_perfs }},
        Some(Err(err)) => rsx! {p { class:"pending-messages", "Oulah... Erreur\n{err}" }},
        None => rsx! {p { class:"pending-messages", "Patience..." }}    }
}

#[component]
fn Banner(on_click: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            class: "reload-button",
            onclick: move |evt| on_click.call(evt),
            img {
                class: "reload-icon",
                src: RELOAD_SVG,
                width: 24,
                height: 24,
                alt: "Recharger",
            }
        }
    }
}

#[component]
fn PerfCards(user_perfs: Vec<UserPerformance>) -> Element {
    rsx! {
        for (i, user) in user_perfs.iter().enumerate() {
            {
                let games_lost = user.games_played - user.games_won;
                let ratio = (100*user.games_won).checked_div(user.games_played).unwrap_or(0);
                let podium_class = match i {
                    0 => "first",
                    1 => "second",
                    2 => "third",
                    _ => ""
                };
                rsx! {
                    div {
                        class: "card {podium_class}",
                        div { class: "top-left", "{user.name}" }
                        div { class: "top-right", "{user.elo}" }
                        div { class: "bottom-left",
                            "Total:"
                            span { class: "card-digits", "{user.games_played}"}
                            "Gagnées:"
                            span { class: "card-digits", "{user.games_won}"}
                            "Perdues:"
                            span { class: "card-digits", "{games_lost}"}
                        }
                        div { class: "bottom-right", "{ratio}%"}
                    }
                }
            }
        }
    }
}
