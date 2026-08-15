use crate::api::auth;
use crate::client::route::Route;
use dioxus::prelude::*;

const LOGIN_CSS: Asset = asset!("/assets/login.css");

#[component]
pub fn Login() -> Element {
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_loading = use_signal(|| false);
    let mut show_forgot_popup = use_signal(|| false);
    let nav = navigator();

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let user_email = email();
        let user_password = password();

        spawn(async move {
            is_loading.set(true);
            error_msg.set(None);

            match auth::login(user_email, user_password).await {
                Ok(Ok(())) => {
                    nav.push(Route::Leaderboard);
                }
                Ok(Err(err)) => {
                    error_msg.set(Some(err));
                }
                Err(err) => {
                    error_msg.set(Some(format!("SERVER ERROR > {}", err)));
                }
            }
            is_loading.set(false);
        });
    };

    rsx! {
        document::Stylesheet { href : LOGIN_CSS }

        div { class: "login-container",
            h2 { "Leaderboule" }
            p { "Connexion via Helloasso"}
            p { "1. Entre l'email utilisé pour ton adhésion helloasso"}
            p { "2. Si c'est ta première connexion, choisis un mot de passe, tu pourras le modifier plus tard"}
            p { "3. Connecte-toi"}
            p { "Entrez votre email d'adhérent :" }

            form { onsubmit: on_submit,
                input {
                    r#type: "email",
                    placeholder: "exemple@domaine.fr",
                    value: "{email}",
                    oninput: move |e| email.set(e.value()),
                    required: true,
                }
                input {
                    r#type: "password",
                    value: "{password}",
                    oninput: move |e| password.set(e.value()),
                    required: true,
                }
                button {
                    type: "submit",
                    disabled: is_loading(),
                    {if is_loading() { "Vérification..." } else { "Se connecter" }}
                }
            }

            if let Some(err) = error_msg() {
                p { class: "error-message", style: "color: red;", "{err}" }
            }

            div { onclick: move |_| show_forgot_popup.toggle(),
                "J'ai oublié mon mot de passe"
            }

            if show_forgot_popup() {
                p {
                    "Si vous avez oublié ou perdu votre mot de passe, envoyez un mail à petanquinsaclub@asso-insa-lyon.fr avec votre adresse email d'adhérent !"
                }
            }
        }
    }
}
