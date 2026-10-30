use crate::api::auth;
use crate::client::route::Route;
use dioxus::prelude::*;

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
        div { class: "login-outer-container route-container",
            h1 { class: "login-title", "Leaderboule" }
            div { class: "login-inner-container",
                p { "C'est ma première connexion :" }
                div { class: "instruction-container",
                    span { class: "instruction-bullet", "1" }
                    a { href: "https://www.helloasso.com/associations/petanqu-insa-club", "Adhère à l'assocation Pétanqu'INSA club via Helloasso"}
                    span { class: "instruction-bullet", "2" }
                    span { "Entre l'e-mail utilisé pour ton adhésion"}
                    span { class: "instruction-bullet", "3" }
                    span { "Choisis un mot de passe"}

                }
                p { "Entrez votre email d'adhérent :" }
                form {
                    class: "login-form",
                    onsubmit: on_submit,
                    input { class: "login-input",
                        r#type: "email",
                        placeholder: "petanque@petanque.fr",
                        autocomplete: "email",
                        value: "{email}",
                        oninput: move |e| email.set(e.value()),
                        required: true,
                    }
                    input { class: "login-input",
                        r#type: "password",
                        placeholder: "Mot de passe",
                        autocomplete: "current-password",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                        required: true,
                    }
                    button { class: "login-button",
                        type: "submit",
                        disabled: is_loading(),
                        {if is_loading() { "Vérification..." } else { "Se connecter" }}
                    }
                }

                if let Some(err) = error_msg() {
                    p { class: "error-message", role: "alert", "{err}" }
                }

                button { class: "forgot-password-button",
                    r#type: "button",
                    onclick: move |_| show_forgot_popup.toggle(),
                    "J'ai oublié mon mot de passe"
                }

                if show_forgot_popup() {
                    div { class: "forgotten-password",
                        span { "Si vous avez oublié ou perdu votre mot de passe, envoyez un mail à " }
                        span { class: "petanque-email", "petanquinsaclub@asso-insa-lyon.fr"}
                        span { " depuis votre adresse mail d'adhérent Helloasso, avec simplement comme objet \" Mot de passe oublié \" et on s'occupera de tout !"}
                    }
                }
            }
        }
    }
}
