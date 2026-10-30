use dioxus::prelude::*;

#[component]
pub fn PageNotFound(segments: Vec<String>) -> Element {
    let mut filler = use_signal(String::new);
    use_future(move || async move {
        loop {
            filler.write().push_str("404 ");
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(50).await;
        }
    });
    rsx! {
        div { class: "not-found route-container",
            p { class: "not-found-code", "404" }
            h1 { "Page non trouvée" }
            p { class: "not-found-message", "Désolé, cette page n'existe pas." }
            p { class: "not-found-filler", "{filler}" }
        }
    }
}
