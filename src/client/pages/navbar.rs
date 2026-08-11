use crate::{api::auth, client::route::Route};
use dioxus::prelude::*;

const NAVBAR_CSS: Asset = asset!("assets/navbar.css");
const HAMBURGER_SVG: Asset = asset!("assets/hamburger.svg");
const LEADERBOARD_SVG: Asset = asset!("assets/leaderboard.svg");
const ACCOUNT_SVG: Asset = asset!("assets/account.svg");

#[component]
pub fn Navbar() -> Element {
    // Authentification management
    let nav = navigator();
    // let on_logout = move |_| async move {
    //     let _ = auth::logout().await;
    //     nav.push(Route::Login);
    // };
    let user_resource = use_server_future(auth::get_current_user)?;
    use_effect(move || match user_resource() {
        Some(Ok(None)) | Some(Err(_)) => {
            nav.replace(Route::Login);
        }
        _ => {}
    });
    let username = match user_resource() {
        Some(Ok(Some(user))) => user.email,
        Some(Ok(None)) => "Redirection...".to_string(),
        Some(Err(_)) => "ERREUR".to_string(),
        None => "Chargement...".to_string(), // supposedly unreachable
    };

    let mut is_open = use_signal(|| false);
    let current_route = use_route::<Route>();
    let on_link_click = move |_| {
        #[cfg(target_arch = "wasm32")] // make rust-analyzer ignore the block
        #[cfg(feature = "web")]
        if let Some(window) = web_sys::window() {
            if let Ok(Some(mq)) = window.match_media("(max-width: 50rem)") {
                if mq.matches() {
                    is_open.set(false);
                }
            }
        }
    };

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
                div { class: "routes-container classic-routes",
                    Link {
                        class: if is_open() { "route-entry open" } else { "route-entry" },
                        to: Route::Leaderboard {},
                        active_class: "active-route",
                        onclick: on_link_click,
                        img {
                            class: "route-icon",
                            src: LEADERBOARD_SVG,
                            width: 24,
                            height: 24,
                        }
                        {Route::Leaderboard.as_str()}
                    }
                }
                div { class: "routes-container account-route",
                    Link {
                        class: if is_open() { "route-entry open" } else { "route-entry" },
                        to: Route::Account {},
                        active_class: "active-route",
                        onclick: on_link_click,
                        img {
                            class: "route-icon",
                            src: ACCOUNT_SVG,
                            width: 24,
                            height: 24,
                        }
                        {username}
                    }
                }
            }

            // Page content injected here
            {if let Some(Err(e)) = user_resource() {
                rsx!{ p {"Erreur : {e.to_string()}"} }
            } else {
                rsx!{
                    main { class: if is_open() { "main-content open" } else { "main-content" },
                    Outlet::<Route> {}}
                }
            }}
        }
}
