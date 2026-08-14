use dioxus::prelude::*;

use crate::{api::auth, client::route::Route};

#[component]
pub fn Logout() -> Element {
    let nav = navigator();

    // Trigger logout as soon as this page is visited
    use_effect(move || {
        spawn(async move {
            let _ = auth::logout().await;
            nav.replace(Route::Login {}); // Redirects to Login page
        });
    });

    rsx! {
        p { class: "abnormal-state-message", "Deconnexion en cours..." }
    }
}
