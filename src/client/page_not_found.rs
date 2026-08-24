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
        h1 { "404 Page non trouvée" }
        h3 { "Désolé" }
        p { "{filler}" }
    }
}
