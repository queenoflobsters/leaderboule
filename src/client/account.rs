use dioxus::prelude::*;

use crate::{
    api::{
        auth,
        db::current_user::{self, UserProfile},
    },
    client::route::Route,
};

const ACCOUNT_CSS: Asset = asset!("/assets/style/account.css");
const ACCOUNT_SVG: Asset = asset!("/assets/icons/account.svg");
const TROPHY_SVG: Asset = asset!("assets/icons/trophy.svg");
const BROKEN_HEART_SVG: Asset = asset!("assets/icons/broken-heart.svg");
const TOTAL_SVG: Asset = asset!("assets/icons/total.svg");
const PERCENT_SVG: Asset = asset!("assets/icons/percent.svg");
const HASH_SVG: Asset = asset!("assets/icons/hash.svg");

#[component]
pub fn Account() -> Element {
    let profile_hook = use_server_future(current_user::get_profile)?;
    let nav = use_navigator();
    let mut error_msg = use_signal(String::new);
    let profile = match profile_hook() {
        Some(Ok(profile)) => profile,
        Some(Err(e)) => {
            error_msg.set(e.to_string());
            let mut temp = UserProfile::default();
            temp.username = "ERREUR".to_string();
            temp
        }
        None => {
            let mut temp = UserProfile::default();
            temp.username = "Chargement...".to_string();
            temp
        },
    };
    let mut logout_button_message = use_signal(|| "Déconnexion");
    let mut logout_button_loading = use_signal(|| false);
    let logout_fn = move |_evt| {
        let nav = nav.clone();
        spawn(async move {
            logout_button_message.set("Chargement...");
            logout_button_loading.set(true);
            match auth::logout().await {
                Ok(()) => _ = nav.push(Route::Login),
                Err(e) => {
                    error_msg.set(e.to_string());
                    logout_button_message.set("Déconnexion");
                    logout_button_loading.set(false)
                },
            }
        });
    };

    rsx! {
        document::Stylesheet { href: ACCOUNT_CSS }
        div { class: "account-container",
            img { class: "account-icon", src: ACCOUNT_SVG, }
            p { class: "username-title",
                {profile.username}
            }
            div { class: "profile-container",
                span { class: "primary-stat",
                    span { class: "primary-stat-title", "Elo"}
                    span { class: "stat-elo", "{profile.elo}"}
                }
                span { class: "secondary-stat",
                    "Record  "
                    span { class: "stat-elo", "{profile.best_elo}"}
                }
                span { class: "secondary-stat",
                    "Classement"
                    img { class: "stat-rank stat-icon", src: HASH_SVG }
                    span { class: "stat-rank", "{profile.rank.unwrap_or(0)}" }
                }
            }
            div { class: "profile-container",
                span { class: "primary-stat", 
                    span { class: "primary-stat-title", "Parties jouées" }
                    img { class: "stat-icon stat-games-played", src: TOTAL_SVG }
                    span { class : "stat-games-played", "{profile.games_played}" }
                }
                span { class: "secondary-stat",
                    "Gagnées"
                    img { class: "stat-icon stat-games-won", src: TROPHY_SVG }
                    span { class: "stat-games-won", "{profile.games_won}" }
                }
                span { class: "secondary-stat",
                    "Perdues"
                    img { class: "stat-icon stat-games-lost", src: BROKEN_HEART_SVG }
                    span { class: "stat-games-lost", "{profile.games_lost}" }
                }
                span { class: "secondary-stat",
                    "Ratio"
                    img { class: "stat-icon stat-win-ratio", src: PERCENT_SVG }
                    span { class: "stat-win-ratio", "{profile.win_ratio:.1}" }
                }
            }

            { if !error_msg.is_empty() { rsx! {
                p { class: "error-msg", {error_msg()}}
            }} else { rsx!{} } }

            div { class: "profile-container management-container",
                div { 
                    "e-mail :"
                    span { class: "email", "{profile.email}"}
                }
                button {
                    onclick: logout_fn,
                    disabled: logout_button_loading(),
                    {logout_button_message()}
                }
                
            }
        }
    }
}
