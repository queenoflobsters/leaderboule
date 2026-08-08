use crate::client::Route;
use dioxus::prelude::*;

const NAVBAR_CSS: Asset = asset!("assets/navbar.css");
const HAMBURGER_SVG: Asset = asset!("assets/hamburger.svg");
const LEADERBOARD_SVG: Asset = asset!("assets/leaderboard.svg");

#[component]
pub fn Navbar() -> Element {
    let mut is_open = use_signal(|| false);
    let current_route = use_route::<Route>();

    rsx! {
        document::Stylesheet { href: NAVBAR_CSS }

        // Top bar fixed at top
        header { class: "top-bar",
            button {
                class: if is_open() {"hamburger open"} else {"hamburger"},
                aria_label: "Open menu",
                onclick: move |_| is_open.toggle(),
                img {
                    src: HAMBURGER_SVG,
                    width: 28,
                    height: 28,
                    alt: "Menu",
                }
            }
            div { class: "route-title", {current_route.as_str()} }
        }

        // Sliding Sidebar
        nav { class: if is_open() { "sidebar open" } else { "sidebar" },
            ul { class: "nav-links",
                li {
                    Link {
                        class: if is_open() { "route-link open" } else { "route-link" },
                        to: Route::Leaderboard {},
                        active_class: "active-route",
                        onclick: move |_| is_open.set(false),
                        img {
                            class: "route-icon",
                            src: LEADERBOARD_SVG,
                            width: 24,
                            height: 24,
                        }
                        "{Route::Leaderboard.as_str()}"
                    }
                }
            }
        }

        // Page content injected here
        main { class: if is_open() { "main-content open" } else { "main-content" },
            Outlet::<Route> {}
        }
    }
}
