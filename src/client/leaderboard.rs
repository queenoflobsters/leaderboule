use crate::{api::db::global::{self, LeaderboardSortMethod, LeaderboardUserCard}, client::components::{page_switcher::PageSwitcher, search_banner::SearchBanner}};
use dioxus::{prelude::*};

const LEADERBOARD_CSS: Asset = asset!("assets/style/leaderboard.css");
const TROPHY_SVG: Asset = asset!("assets/icons/trophy.svg");
const BROKEN_HEART_SVG: Asset = asset!("assets/icons/broken-heart.svg");
const TOTAL_SVG: Asset = asset!("assets/icons/total.svg");
const PERCENT_SVG: Asset = asset!("assets/icons/percent.svg");
const HASH_SVG: Asset = asset!("assets/icons/hash.svg");

#[component]
pub fn Leaderboard() -> Element {
    let mut refresh_count = use_signal(|| 0);
    let mut search_query = use_signal(String::new);
    let mut sort_method = use_signal(|| LeaderboardSortMethod::Elo);
    let mut current_page = use_signal(|| 0);

    rsx! {
        document::Stylesheet { href: LEADERBOARD_CSS }

        div { class: "leaderboard-container route-container",

            SearchBanner {
                current_sort_method: sort_method(),
                on_search: move |query| {
                    current_page.set(0);
                    search_query.set(query);
                },
                on_sort_change: move |new_sort| {
                    current_page.set(0);
                    sort_method.set(new_sort);
                },
                on_refresh: move |_| {
                    refresh_count += 1;
                },
            }

            div { class: "cards",
                SuspenseBoundary {
                    fallback: |_| rsx! { p { class: "abnormal-state-message", "Patience..." } },
                    UserCardsList { search_query, sort_method, current_page, refresh_count,  }
                }
            }

            PageSwitcher {
                current_page: current_page(),
                current_page_change: move |val| current_page.set(val)
            }
        }
    }
}


#[component]
fn UserCardsList(
    search_query: ReadSignal<String>,
    current_page: ReadSignal<u64>,
    sort_method: ReadSignal<LeaderboardSortMethod>,
    refresh_count: ReadSignal<u64>,
) -> Element {
    let cards_resource = use_server_future(move || {
        let _ = refresh_count();
        let q = search_query();
        let s = sort_method();
        let p = current_page();
        async move { global::get_leaderboard_cards(q, s, p).await }
    })?;
    let do_podium = search_query().is_empty() && current_page() == 0;
    match cards_resource() {
        Some(Ok(user_card)) if user_card.is_empty() => rsx! {
            p { class: "abnormal-state-message", "Aucun joueur trouvé"}
        },
        Some(Ok(user_perfs)) => rsx! {
            for (i, user) in user_perfs.iter().enumerate() {{
                let use_index = if do_podium { Some(i) } else { None };
                rsx! {UserCard { key: "{user.username}", use_index, user: user.clone() }}
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
fn UserCard(use_index: Option<usize>, user: LeaderboardUserCard) -> Element {
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
                    img { class: "icon", src: TROPHY_SVG}
                    "{user.games_won}"
                }
                span { class: "stat-container games-lost",
                    img { class: "icon", src: BROKEN_HEART_SVG}
                    "{user.games_lost}"
                }
                span { class: "stat-container games-played",
                    img { class: "icon", src: TOTAL_SVG}
                    "{user.games_played}"
                }
                span { class: "stat-container ratio",
                    img { class: "icon", src: PERCENT_SVG}
                    "{user.win_ratio:.1}"
                }
            }
            div { class : "player-rank",
                img { class: "icon", src: HASH_SVG}
                "{user.rank.unwrap_or(0)}"
            }
        }
    }
}


