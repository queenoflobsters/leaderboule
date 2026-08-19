use dioxus::prelude::*;

const NEW_GAME_CSS: Asset = asset!("assets/style/new_game.css");
const MINUS_SVG: Asset = asset!("assets/icons/minus.svg");
const PLUS_SVG: Asset = asset!("assets/icons/plus.svg");

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
        }
    }
}

#[component]
fn TeamEntry(title: String, score: u64, score_change: EventHandler<u64>) -> Element {
    rsx! {
        div { class: "team-entry",
            div { class: "team-entry-banner",
                h2 { {title} }

            }
            ScoreInput {
                score,
                score_change
            }
        }
    }
}

#[component]
fn ScoreInput(score: u64, score_change: EventHandler<u64>) -> Element {
    rsx! {
        div { class: "score-input",
            button { class: "score-input-buttons",
                onclick: move |_| if score > 0 {
                    score_change(score -1);
                },
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
            button { class: "score-input-buttons", 
                onclick: move |_| if score < 13 {score_change(score + 1)},
                img { class: "score-input-icon", src: PLUS_SVG}
            }

        }
    }
}
