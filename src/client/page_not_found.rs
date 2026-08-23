use dioxus::prelude::*;

use crate::api::utils;

#[component]
pub fn PageNotFound(segments: Vec<String>) -> Element {
    let mut filler = use_signal(String::new);
    use_future(move || async move {
        loop {
            filler.write().push_str("404 ");
            utils::sleep_ms(30).await;
        }
    });
    rsx! {
        h1 { "404 Page non trouvée" }
        h3 { "Désolé" }
        p { "{filler}" }
    }
}
