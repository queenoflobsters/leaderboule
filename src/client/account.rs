use dioxus::prelude::*;

use crate::{
    api::{
        auth,
        db::current_user::{self, UserProfile},
        utils,
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

fn hard_reload() {
    #[cfg(target_arch = "wasm32")] // make rust-analyzer ignore the block
    #[cfg(feature = "web")]
    if let Some(window) = web_sys::window() {
        _ = window.location().reload();
    }
}

#[component]
pub fn Account() -> Element {
    let profile_hook = use_server_future(current_user::get_profile)?;
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

    rsx! {
            document::Stylesheet { href: ACCOUNT_CSS }

            div { class: "account-container route-container",
                AccountTitle { username: &profile.username, member_since: profile.member_since }
                ProfileStats { profile: profile.clone() }

                { if !error_msg.is_empty() { rsx! {
                    p { class: "error-message", {error_msg()}}
                }} else { rsx!{} } }

                Management { error_msg, email: &profile.email }

        }
    }
}

#[component]
fn AccountTitle(username: String, member_since: u64) -> Element {
    let date = utils::format_date(member_since);
    rsx! {
        img { class: "account-icon", src: ACCOUNT_SVG, }
        p { class: "username-title",
            {username}
        }
        p { class: "member-since-title",
            "Membre depuis {date}"
        }
    }
}

#[component]
fn ProfileStats(profile: UserProfile) -> Element {
    rsx! {
        div { class: "profile-container",
            span { class: "primary-stat",
                span { class: "stat-title", "Points Pétanque"}
                span { class: "stat-elo cool-glow", "{profile.elo}"}
            }
            span { class: "secondary-stat",
                span { class: "stat-title", "Record"}
                span { class: "stat-elo cool-glow", "{profile.best_elo}"}
            }
            span { class: "secondary-stat",
                span { class: "stat-title", "Classement"}
                img { class: "stat-rank cool-glow icon", src: HASH_SVG }
                span { class: "stat-rank cool-glow", "{profile.rank.unwrap_or(0)}" }
            }
        }
        div { class: "profile-container",
            span { class: "primary-stat",
                span { class: "stat-title", "Parties jouées" }
                img { class: "icon stat-games-played cool-glow", src: TOTAL_SVG }
                span { class : "stat-games-played cool-glow", "{profile.games_played}" }
            }
            span { class: "secondary-stat",
                span { class: "stat-title", "Gagnées"}
                img { class: "icon stat-games-won cool-glow", src: TROPHY_SVG }
                span { class: "stat-games-won cool-glow", "{profile.games_won}" }
            }
            span { class: "secondary-stat",
                span { class: "stat-title", "Perdues"}
                img { class: "icon stat-games-lost cool-glow", src: BROKEN_HEART_SVG }
                span { class: "stat-games-lost cool-glow", "{profile.games_lost}" }
            }
            span { class: "secondary-stat",
                span { class: "stat-title", "Ratio"}
                img { class: "icon stat-win-ratio cool-glow", src: PERCENT_SVG }
                span { class: "stat-win-ratio cool-glow", "{profile.win_ratio:.1}" }
            }
        }
    }
}

#[component]
fn Management(error_msg: Signal<String>, email: String) -> Element {
    rsx! {
        div { class: "profile-container management-container",
            div {
                "e-mail :"
                span { class: "email", "{email}"}
            }
            DisconnectButton { error_msg }
            UsernameChanger { error_msg }
            PasswordChanger { error_msg }
        }
    }
}

#[component]
fn DisconnectButton(error_msg: Signal<String>) -> Element {
    let nav = use_navigator();
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
    rsx! {
        button {
            onclick: logout_fn,
            disabled: logout_button_loading(),
            {logout_button_message()}
        }
    }
}

#[component]
fn UsernameChanger(error_msg: Signal<String>) -> Element {
    let mut value = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut message = use_signal(|| "Changer le nom d'utilisateur".to_string());
    let mut show = use_signal(|| false);
    let mut loading = use_signal(|| false);
    let change_username_fn = move |_evt| {
        message.set("Chargement...".to_string());
        loading.set(true);
        spawn(async move {
            match auth::change_username(value.to_string(), password.to_string()).await {
                Ok(Ok(())) => {
                    _ = {
                        message.set("Nom d'utilisateur changé avec succès".to_string());
                        password.set(String::new());
                        value.set(String::new());
                        loading.set(false);
                        hard_reload();
                    }
                }
                Ok(Err(e)) => {
                    message.set(e);
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(e.to_string());
                    message.set("Changer le nom d'utilisateur".to_string());
                    loading.set(false)
                }
            }
        });
    };
    if show() {
        rsx! {
            div { class: "profile-container important-container",
                span { "Nouveau nom d'utilisateur" }
                input {
                    class: "important-input",
                    r#type: "username",
                    placeholder: "Nom d'utilisateur",
                    value: "{value}",
                    oninput: move |e| value.set(e.value()),
                }
                span { "Mot de passe" }
                input {
                    class: "important-input",
                    r#type: "password",
                    placeholder: "Mot de passe",
                    value: "{password}",
                    oninput: move |e| password.set(e.value()),
                }
                button { class: "important-button",
                    disabled: loading(),
                    onclick: change_username_fn,
                    "{message}"
                }
            }
        }
    } else {
        rsx! {
            button { class: "important-button",
                onclick: move |_| show.set(true),
                "{message}"
            }
        }
    }
}

#[component]
fn PasswordChanger(error_msg: Signal<String>) -> Element {
    let mut password_old = use_signal(String::new);
    let mut password_new = use_signal(String::new);
    let mut message = use_signal(|| "Changer le mot de passe".to_string());
    let mut show = use_signal(|| false);
    let mut loading = use_signal(|| false);
    let change_password_fn = move |_evt| {
        message.set("Chargement...".to_string());
        loading.set(true);
        spawn(async move {
            match auth::change_password(password_old.to_string(), password_new.to_string()).await {
                Ok(Ok(())) => {
                    _ = {
                        message.set("Mot de passe changé avec succès".to_string());
                        password_new.set(String::new());
                        password_old.set(String::new());
                        hard_reload();
                    }
                }
                Ok(Err(e)) => {
                    message.set(e);
                }
                Err(e) => {
                    error_msg.set(e.to_string());
                    message.set("Changer le mot de passe".to_string());
                }
            }
            loading.set(false)
        });
    };

    if show() {
        rsx! {
            div { class: "profile-container important-container",
                span { "Mot de passe actuel" }
                input {
                    class: "important-input",
                    r#type: "password",
                    placeholder: "Mot de passe actuel",
                    value: "{password_old}",
                    oninput: move |e| password_old.set(e.value()),
                }
                span { "Nouveau mot de passe" }
                input {
                    class: "important-input",
                    r#type: "password",
                    placeholder: "Nouveau mot de passe",
                    value: "{password_new}",
                    oninput: move |e| password_new.set(e.value()),
                }
                button { class: "important-button",
                    disabled: loading(),
                    onclick: change_password_fn,
                    "{message}"
                }
            }
        }
    } else {
        rsx! {
            button { class: "important-button",
                onclick: move |_| show.set(true),
                "{message}"
            }
        }
    }
}
