use dioxus::prelude::*;

const ACCOUNT_CSS: Asset = asset!("/assets/account.css");

#[component]
pub fn Account() -> Element {
    rsx! {
        document::Stylesheet { href: ACCOUNT_CSS }
        div {
            class: "account-container",
            "Eh coucouc les copains"
        }
    }
}
