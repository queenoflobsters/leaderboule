use dioxus::{core::Task, prelude::*};

use crate::api::{
    db::global::{self, UserSearchItem},
    utils,
};

const NEW_GAME_CSS: Asset = asset!("assets/style/new_game.css");
const MINUS_SVG: Asset = asset!("assets/icons/minus.svg");
const PLUS_SVG: Asset = asset!("assets/icons/plus.svg");
const CLOSE_SVG: Asset = asset!("assets/icons/close.svg");


#[component]
pub fn NewGame() -> Element {
    let mut left_score = use_signal(|| 13);
    let mut right_score = use_signal(|| 0);

    rsx! {
        document::Stylesheet { href : NEW_GAME_CSS }
        div { class: "new-game-container",
            div { class: "team-entry-container",
                TeamEntry {
                    title: "Équipe Gagnante",
                    score: left_score(),
                    score_change: move |val| left_score.set(val),
                }
                TeamEntry {
                    title: "Équipe Perdante",
                    score: right_score(),
                    score_change: move |val| right_score.set(val),
                }
            }
            button { class: "validate-button",
                "Valider"
            }
        }
    }
}

#[component]
fn TeamEntry(title: String, score: u64, score_change: EventHandler<u64>) -> Element {
    rsx! {
        div { class: "team-entry",
            h2 { class: "team-entry-title",
                {title}
            }
            ScoreInput {
                score,
                score_change
            }
            PlayerSelector {  }
        }
    }
}

#[component]
fn ScoreInput(score: u64, score_change: EventHandler<u64>) -> Element {
    rsx! {
        div { class: "score-input",
            button { class: "score-input-buttons",
                onclick: move |_| if score > 0 { score_change(score -1) },
                img { class: "score-input-icon", src: MINUS_SVG}
            }
            input { class: "score-input-field",
                r#type: "text",
                inputmode: "numeric",
                pattern: "[0-9]*",
                placeholder: "0",
                value: "{score}",
                oninput: move |evt| {
                    let num = evt
                        .value()
                        .chars()
                        .filter(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<u64>()
                        .unwrap_or(0); // !!! WARNING DANGEROUS UNWRAP
                    score_change.call(num.min(13))
                },
            }
            button { class: "score-input-button",
                onclick: move |_| if score < 13 { score_change(score + 1) },
                img { class: "score-input-icon", src: PLUS_SVG}
            }

        }
    }
}

#[component]
fn PlayerSelector() -> Element {
    let mut search_query = use_signal(String::new);
    let mut debounce_task = use_signal(|| None::<Task>);
    let on_input = move |evt: Event<FormData>| {
        let new_val = evt.value();
        if let Some(task) = debounce_task.take() {
            task.cancel();
        }
        let task = spawn(async move {
            utils::sleep_ms(300).await;
            search_query.set(new_val);
        });
        debounce_task.set(Some(task));
    };
    rsx! {
        div { class: "player-selector-container",
            div { class: "player-selector-banner",
                input { class: "player-selector-search-bar",
                    r#type: "search",
                    placeholder: "Rechercher...",
                    value: "{search_query}",
                    oninput: on_input,
                }
                button { class: "player-selector-clear",
                    onclick: move |_| search_query.set(String::new()),
                    img { class: "player-selector-clear-icon", src: CLOSE_SVG}

                }

            }

            {if !search_query().is_empty() { rsx! {
                div { class: "player-box player-selector-search-box",
                    SuspenseBoundary {
                        fallback:|_| rsx! { p { class: "abnormal-state-message", "Patience..." } },
                        PlayerSearchBox { search_query }
                    }
                }
            }} else { rsx!{} }}


            div { class: "players-selected",
                p {
                    "BONJOUR JE SUIS UNE BOITE LALALALALA LALALALALA LALALALALA LALALALALA LALALALALA LALALALALA LALALALALA LALALALALA LALALALALA LALALALALA ",
                }
            }
        }
    }
}

#[component]
fn PlayerSearchBox(search_query: ReadSignal<String>) -> Element {
    let cards_resource = use_server_future(move || {
        let s = search_query();
        async move { global::search_user(s).await }
    })?;

    match cards_resource() {
        Some(Ok(players)) if players.is_empty() => rsx! {
            p { class: "abnormal-state-message", "Aucun joueur trouvé"}
        },
        Some(Ok(players)) => rsx! {
            for item in players {
                { rsx! { UserSearchCard { item } } }
            }
        },
        Some(Err(e)) => rsx! {
            p { class: "abnormal-state-message", "Oulah... Erreur :\n{e}" }
        },
        None => rsx! {
            p { class: "abnormal-state-message", "Patience..." }
        },
    }
}

#[component]
fn UserSearchCard(item: UserSearchItem) -> Element {
    rsx! {
        div { class: "user-search-card",
            div { class: "user-search-card-username",
                "{item.username}"
            }
            div { class: "user-search-card-elo",
                "{item.elo}"
            }
        }
    }
}
