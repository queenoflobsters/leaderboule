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

#[component]
pub fn Account() -> Element {
    let profile_hook = use_server_future(current_user::get_profile)?;
    let nav = use_navigator();
    let profile = match profile_hook() {
        Some(Ok(profile)) => profile,
        _ => UserProfile::default(),
    };
    let logout_closure = move |_evt| {
        let nav = nav.clone();
        spawn(async move {
            match auth::logout().await {
                Ok(()) => _ = nav.push(Route::Login),
                Err(err) => error!("Logout failed {:?}", err),
            }
        });
    };
    let logout_button_message = use_signal(|| "Déconnexion");
    let logout_button_loading = use_signal(|| false);

    rsx! {
        document::Stylesheet { href: ACCOUNT_CSS }
        div { class: "account-container",
            img { class: "account-icon", src: ACCOUNT_SVG, }
            p { class: "username-title",
                {profile.username}
            }
            button {
                onclick: logout_closure,
                disabled: logout_button_loading(),
                {logout_button_message()}
            }
            p {
                "elo : {profile.elo}"
            }
            p {
                "Record : {profile.best_elo}"
            }
            p {
                "Classement : {profile.rank.unwrap_or(0)}"
            }
            p {
                "Parties jouées : {profile.games_played}"
            }
            p {
                "Parties gagnées : {profile.games_won}"
            }
            p {
                "Parties perdues : {profile.games_lost}"
            }
            p {
                "Ratio de victoire : {profile.win_ratio:.1}"
            }
            p {
                "E-Mail : {profile.email}"
            }

        }
    }
}
