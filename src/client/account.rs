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
        }
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
                }
            }
        });
    };

    let mut change_username_value = use_signal(String::new);
    let mut change_username_password = use_signal(String::new);
    let mut change_username_message = use_signal(|| "Changer le nom d'utilisateur".to_string());
    let mut change_username_show = use_signal(|| false);
    let mut change_username_loading = use_signal(|| false);
    let change_username_fn = move |_evt| {
        spawn(async move {
            change_username_message.set("Chargement...".to_string());
            change_username_loading.set(true);
            match auth::change_username(
                change_username_value.to_string(),
                change_username_password.to_string(),
            )
            .await
            {
                Ok(Ok(())) => {
                    _ = {
                        change_username_message.set("Nom d'utilisateur changé avec succès".to_string());
                        change_username_password.set(String::new());
                        change_username_value.set(String::new());
                        change_username_loading.set(false);
                    }
                }
                Ok(Err(e)) => {
                    change_username_message.set(e);
                    change_username_loading.set(false);
                }
                Err(e) => {
                    error_msg.set(e.to_string());
                    change_username_message.set("Changer le nom d'utilisateur".to_string());
                    change_username_loading.set(false)
                }
            }
        });
    };
    let mut change_password_old = use_signal(String::new);
    let mut change_password_new = use_signal(String::new);
    let mut change_password_message = use_signal(|| "Changer le mot de passe".to_string());
    let mut change_password_show = use_signal(|| false);
    let mut change_password_loading = use_signal(|| false);
    let change_password_fn = move |_evt| {
        spawn(async move {
            change_password_message.set("Chargement...".to_string());
            change_password_loading.set(true);
            match auth::change_password(
                change_password_old.to_string(),
                change_password_new.to_string(),
            )
            .await
            {
                Ok(Ok(())) => {
                    _ = {
                        change_password_message.set("Mot de passe changé avec succès".to_string());
                        change_password_new.set(String::new());
                        change_password_old.set(String::new());
                        change_password_loading.set(false);
                    }
                }
                Ok(Err(e)) => {
                    change_password_message.set(e);
                    change_password_loading.set(false);
                }
                Err(e) => {
                    error_msg.set(e.to_string());
                    change_password_message.set("Changer le mot de passe".to_string());
                    change_password_loading.set(false)
                }
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
                { if change_username_show() { rsx! {
                    div { class: "profile-container important-container",
                        span { "Nouveau nom d'utilisateur" }
                        input {
                            class: "important-input",
                            r#type: "username",
                            placeholder: "Nom d'utilisateur",
                            value: "{change_username_value}",
                            oninput: move |e| change_username_value.set(e.value()),
                        }
                        span { "Mot de passe" }
                        input {
                            class: "important-input",
                            r#type: "password",
                            placeholder: "Mot de passe",
                            value: "{change_username_password}",
                            oninput: move |e| change_username_password.set(e.value()),
                        }
                        button { class: "important-button",
                            disabled: change_username_loading(),
                            onclick: change_username_fn,
                            "{change_username_message}"
                        }
                    }
                }} else { rsx!{
                    button { class: "important-button",
                        onclick: move |_| change_username_show.set(true),
                        "{change_username_message}"
                    }
                }}}
                { if change_password_show() { rsx! {
                    div { class: "profile-container important-container",
                        span { "Mot de passe actuel" }
                        input {
                            class: "important-input",
                            r#type: "password",
                            placeholder: "Mot de passe actuel",
                            value: "{change_password_old}",
                            oninput: move |e| change_password_old.set(e.value()),
                        }
                        span { "Nouveau mot de passe" }
                        input {
                            class: "important-input",
                            r#type: "password",
                            placeholder: "Nouveau mot de passe",
                            value: "{change_password_new}",
                            oninput: move |e| change_password_new.set(e.value()),
                        }
                        button { class: "important-button",
                            disabled: change_password_loading(),
                            onclick: change_password_fn,
                            "{change_password_message}"
                        }
                    }
                }} else { rsx!{
                    button { class: "important-button",
                        onclick: move |_| change_password_show.set(true),
                        "{change_password_message}"
                    }
                }}}
            }
        }
    }
}
