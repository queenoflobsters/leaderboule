use dioxus::prelude::*;

#[component]
pub fn PageSwitcher(current_page: u64, current_page_change: EventHandler<u64>) -> Element {
    rsx! {
        div { class: "page-switcher-container",
            button { class: "page-switcher-button",
                r#type: "button",
                aria_label: "Première page",
                disabled: current_page == 0,
                onclick: move |_| current_page_change.call(0),
                "<<<"
            }
            button { class: "page-switcher-button",
                r#type: "button",
                aria_label: "Page précédente",
                disabled: current_page == 0,
                onclick: move |_| if current_page > 0 { current_page_change.call(current_page - 1) },
                "<<"
            }
            div { class: "page-switcher-text", aria_live: "polite",
                "{current_page+1}"
            }
            button { class: "page-switcher-button",
                r#type: "button",
                aria_label: "Page suivante",
                onclick: move |_| current_page_change.call(current_page + 1),
                ">>"
            }
        }
    }
}
