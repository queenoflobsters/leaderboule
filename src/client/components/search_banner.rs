use dioxus::{core::Task, prelude::*};

use crate::api::db::global::LeaderboardSortMethod;

const RELOAD_SVG: Asset = asset!("assets/icons/reload.svg");
const SORT_SVG: Asset = asset!("assets/icons/sort.svg");

#[component]
pub fn SearchBanner(
    current_sort_method: LeaderboardSortMethod,
    on_search: EventHandler<String>,
    on_sort_change: EventHandler<LeaderboardSortMethod>,
    on_refresh: EventHandler<()>,
) -> Element {
    let mut show_sort_menu = use_signal(|| false);
    let mut debounce_task = use_signal(|| None::<Task>);
    let sort_active = current_sort_method != LeaderboardSortMethod::Elo;
    let on_input = move |evt: Event<FormData>| {
        let new_val = evt.value();
        if let Some(task) = debounce_task.take() {
            task.cancel();
        }
        let task = spawn(async move {
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(300).await;
            on_search.call(new_val);
        });
        debounce_task.set(Some(task));
    };
    rsx! {
        div { class: "search-banner",
            div { class: "banner",
                input {
                    class: "search-input",
                    r#type: "search",
                    placeholder: "Rechercher...",
                    aria_label: "Rechercher un joueur",
                    oninput: on_input,
                }

                button {
                    class: if sort_active { "sort-button active" } else { "sort-button" },
                    r#type: "button",
                    aria_label: "Trier le classement",
                    onclick: move |_| show_sort_menu.toggle(),
                    img { class: "banner-icon", src: SORT_SVG, alt: "" }
                }
                button {
                    class: "reload-button",
                    r#type: "button",
                    aria_label: "Recharger le classement",
                    onclick: move |_| on_refresh(()),
                    img { class: "banner-icon", src: RELOAD_SVG, alt: "" }
                }
            }

            { if show_sort_menu() { rsx ! {
                div { class: "sort-menu",
                    button { class: "sort-menu-item",
                        r#type: "button",
                        onclick: move |_| {
                            on_sort_change.call(LeaderboardSortMethod::Elo);
                            show_sort_menu.set(false);
                        },
                        "Elo"
                    }
                    button { class: "sort-menu-item",
                        r#type: "button",
                        onclick: move |_| {
                            on_sort_change.call(LeaderboardSortMethod::GamesPlayed);
                            show_sort_menu.set(false);
                        },
                        "Parties jouées"
                    }
                    button { class: "sort-menu-item",
                        r#type: "button",
                        onclick: move |_| {
                            on_sort_change.call(LeaderboardSortMethod::GamesWon);
                            show_sort_menu.set(false);
                        },
                        "Parties gagnées"
                    }
                    button { class: "sort-menu-item",
                        r#type: "button",
                        onclick: move |_| {
                            on_sort_change.call(LeaderboardSortMethod::WinRatio);
                            show_sort_menu.set(false);
                        },
                        "Ratio de victoires"
                    }
                }
            }} else { rsx! {} }}
        }
    }
}
