use crate::api::db::global::{self, LeaderboardSortMethod, LeaderboardUserCard};
use dioxus::{core::Task, prelude::*};

const LEADERBOARD_CSS: Asset = asset!("assets/leaderboard.css");
const RELOAD_SVG: Asset = asset!("assets/reload.svg");
const SORT_SVG: Asset = asset!("assets/sort.svg");
const TROPHY_SVG: Asset = asset!("assets/trophy.svg");
const BROKEN_HEART_SVG: Asset = asset!("assets/broken-heart.svg");
const TOTAL_SVG: Asset = asset!("assets/total.svg");
const PERCENT_SVG: Asset = asset!("assets/percent.svg");

async fn sleep_ms(millis: u32) {
    // HORRENDOUS call to JS for wait helper
    // TODO fix later
    let _ = document::eval(&format!(
        "await new Promise(resolve => setTimeout(resolve, {millis}));"
    ))
    .await;
}

#[component]
pub fn Leaderboard() -> Element {
    let refresh_count = use_signal(|| 0);
    let search_query = use_signal(String::new);
    let sort_method = use_signal(|| LeaderboardSortMethod::Elo);
    let current_page = use_signal(|| 0);
    let page_size = use_signal(|| 10);

    rsx! {
        document::Stylesheet { href: LEADERBOARD_CSS }

        div { class: "leaderboard-container",

            Banner { search_query, sort_method, current_page, refresh_count }

            div { class: "cards",
                SuspenseBoundary {
                    fallback: |_| rsx! { p { class: "abnormal-state-message", "Patience..." } },
                    // 2. The suspended component receives clean signals
                    CardsList { search_query, sort_method, current_page, page_size, refresh_count,  }
                }
            }
        }
    }
}

#[component]
fn Banner(
    search_query: Signal<String>,
    sort_method: Signal<LeaderboardSortMethod>,
    current_page: Signal<u64>,
    refresh_count: Signal<u64>,
) -> Element {
    let mut show_sort_menu = use_signal(|| false);
    let mut debounce_task = use_signal(|| None::<Task>);
    let sort_active = sort_method() != LeaderboardSortMethod::Elo;
    rsx! {
        div { class: "banner",
            input {
                class: "search-input",
                r#type: "search",
                placeholder: "Rechercher...",
                oninput: move |form_data| {
                    let new_val = form_data.value();
                    if let Some(task) = debounce_task.take() {
                        task.cancel();
                    }
                    let task = spawn(async move {
                        sleep_ms(300).await;
                        current_page.set(0);
                        search_query.set(new_val);
                    });
                    debounce_task.set(Some(task));
                },
            }

            button {
                class: if sort_active { "sort-button active" } else { "sort-button" },
                onclick: move |_| show_sort_menu.toggle(),
                img { class: "banner-icon", src: SORT_SVG, alt: "Recharger", }
            }
            button {
                class: "reload-button",
                onclick: move |_| refresh_count += 1,
                img { class: "banner-icon", src: RELOAD_SVG, alt: "Recharger", }
            }
        }

        { if show_sort_menu() { rsx ! {
            div { class: "sort-menu",
                div { class: "sort-menu-item",
                    onclick: move |_| {
                        sort_method.set(LeaderboardSortMethod::Elo);
                        show_sort_menu.set(false);
                    },
                    "Elo"
                }
                div { class: "sort-menu-item",
                    onclick: move |_| {
                        sort_method.set(LeaderboardSortMethod::GamesPlayed);
                        show_sort_menu.set(false);
                    },
                    "Parties jouées"
                }
                div { class: "sort-menu-item",
                    onclick: move |_| {
                        sort_method.set(LeaderboardSortMethod::GamesWon);
                        show_sort_menu.set(false);
                    },
                    "Parties gagnées"
                }
                div { class: "sort-menu-item",
                    onclick: move |_| {
                        sort_method.set(LeaderboardSortMethod::WinRatio);
                        show_sort_menu.set(false);
                    },
                    "Ratio de victoires"
                }
            }
        }} else { rsx! {} }}
    }
}

#[component]
fn CardsList(
    search_query: Signal<String>,
    current_page: Signal<u64>,
    sort_method: Signal<LeaderboardSortMethod>,
    page_size: Signal<u64>,
    refresh_count: Signal<u64>,
) -> Element {
    let cards_resource = use_server_future(move || {
        let _ = refresh_count();
        let q = search_query();
        let s = sort_method();
        let p = current_page();
        let size = page_size();
        async move { global::get_leaderboard_cards(q, s, p, size).await }
    })?;
    let do_podium = search_query().is_empty() && current_page() == 0;
    match cards_resource() {
        Some(Ok(user_card)) if user_card.is_empty() => rsx! {
            p { class: "abnormal-state-message", "Aucun joueur trouvé"}
        },
        Some(Ok(user_perfs)) => rsx! {
            for (i, user) in user_perfs.iter().enumerate() {
                {
                    let use_index = if do_podium { Some(i) } else { None };
                    rsx! {CardItem { key: "{user.username}", use_index, user: user.clone() }}
                }
            }
        },
        Some(Err(err)) => rsx! {
            p { class: "abnormal-state-message", "Oulah... Erreur :\n{err}" }
        },
        None => rsx! {
            p { class: "abnormal-state-message", "Patience..." }
        },
    }
}

#[component]
fn CardItem(use_index: Option<usize>, user: LeaderboardUserCard) -> Element {
    let games_lost = user.games_played.saturating_sub(user.games_won);
    let ratio = (100 * user.games_won)
        .checked_div(user.games_played)
        .unwrap_or(0);
    let podium_class = match use_index {
        Some(0) => "first",
        Some(1) => "second",
        Some(2) => "third",
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
