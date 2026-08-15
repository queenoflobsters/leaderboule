use dioxus::prelude::*;

use crate::{
    api::{
        auth,
        db::current_user::{self, UserProfile},
    },
    client::route::Route,
};

const ACCOUNT_CSS: Asset = asset!("/assets/account.css");
const ACCOUNT_SVG: Asset = asset!("/assets/account.svg");

#[component]
pub fn Account() -> Element {
    let profile_hook = use_server_future(current_user::get_profile)?;
    let nav = use_navigator();
    let profile = match profile_hook() {
        Some(Ok(Some(profile))) => profile,
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
    let games_lost = profile.games_played - profile.games_won;

    rsx! {
        document::Stylesheet { href: ACCOUNT_CSS }
        div { class: "account-container",
            img {
                class: "account-icon",
                src: ACCOUNT_SVG,
                width: 128,
                height: 128,
            }
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
                "Record : TODO"
            }
            p {
                "Classement : TODO"
            }
            p {
                "Record : TODO"
            }
            p {
                "Parties jouées : {profile.games_played}"
            }
            p {
                "Parties gagnées : {profile.games_won}"
            }

            p {
                "Parties perdues : {games_lost}"
            }
            p {
                "E-Mail : {profile.email}"
            }

        }
    }
}
