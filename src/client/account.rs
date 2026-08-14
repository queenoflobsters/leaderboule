use dioxus::prelude::*;

const ACCOUNT_CSS: Asset = asset!("/assets/account.css");
const ACCOUNT_SVG: Asset = asset!("/assets/account.svg");

#[component]
pub fn Account() -> Element {
    rsx! {
        document::Stylesheet { href: ACCOUNT_CSS }
        div { class: "account-container",
            img {
                class: "account-icon",
                src: ACCOUNT_SVG,
                width: 128,
                height: 128,
            }
            p { class: "username-title",
                "{{Username}}"
            }
            
        }
    }
}
