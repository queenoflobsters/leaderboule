use dioxus::prelude::*;

const PAGE_SWITCHER_CSS: Asset = asset!("assets/style/page_switcher.css");

#[component]
pub fn PageSwitcher(current_page: u64, current_page_change: EventHandler<u64>) -> Element {
    rsx! {
        document::Stylesheet { href: PAGE_SWITCHER_CSS }

        div { class: "page-switcher-container",
            button { class: "page-switcher-button",
                onclick: move |_| current_page_change.call(0),
                "<<<"
            }
            button { class: "page-switcher-button",
                onclick: move |_| if current_page > 0 { current_page_change.call(current_page -1 ) }, // WARN underflow problems ?
                "<<"
            }
            div { class: "page-switcher-text",
                "{current_page+1}"
            }
            button { class: "page-switcher-button",
                onclick: move |_| current_page_change.call(current_page + 1),
                ">>"
            }
        }
    }
}
