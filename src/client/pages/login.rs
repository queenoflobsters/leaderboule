use dioxus::prelude::*;
use crate::api::auth;
use crate::client::route::Route;

#[component]
pub fn Login() -> Element {
    let mut email = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_loading = use_signal(|| false);
    let nav = navigator();

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let user_email = email();

        spawn(async move {
            is_loading.set(true);
            error_msg.set(None);

            match auth::login(user_email).await {
                Ok(Ok(())) => {
                    nav.push(Route::Leaderboard);
                }
                Ok(Err(err)) => {
                    error_msg.set(Some(err));
                }
                Err(e) => {
                    error_msg.set(Some(format!("Erreur serveur : {e}")));
                }
            }
            is_loading.set(false);
        });
    };

    rsx! {
        div { class: "login-container",
            h2 { "Connexion via HelloAsso" }
            p { "Entrez votre email d'adhérent :" }

            form { onsubmit: on_submit,
                input {
                    r#type: "email",
                    placeholder: "exemple@domaine.fr",
                    value: "{email}",
                    oninput: move |e| email.set(e.value()),
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
        }
    }
}
