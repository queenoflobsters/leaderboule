use dioxus::prelude::*;

use crate::api::{get_users_performances, UserPerformance};

const LEADERBOARD_CSS: Asset = asset!("assets/leaderboard.css");

#[component]
pub fn Leaderboard() -> Element {
    let user_perfs_hook = use_server_future(get_users_performances)?;
    let user_perfs = match user_perfs_hook() {
        Some(Ok(users)) => users.clone(),
        Some(Err(err)) => panic!("coucou"),
        None => panic!("ah"),
    };
    rsx! {

        document::Stylesheet { href : LEADERBOARD_CSS }
        h1 { "Salut à tous c'est David Lafarge POKEMON" }

        Cards { user_perfs }
    }
}

#[component]
fn Cards(user_perfs: Vec<UserPerformance>) -> Element {
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
                rsx! {div {
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
