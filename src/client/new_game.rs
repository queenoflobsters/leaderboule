///
use crate::{
    api::db::{
        self,
        global::{self, GameSendItem, UserSearchItem},
    },
    client::route::Route,
};
use dioxus::{core::Task, prelude::*};

const NEW_GAME_CSS: Asset = asset!("assets/style/new_game.css");
const MINUS_SVG: Asset = asset!("assets/icons/minus.svg");
const PLUS_SVG: Asset = asset!("assets/icons/plus.svg");
const CLOSE_SVG: Asset = asset!("assets/icons/close.svg");

#[derive(Clone, Default)]
struct ErrorMsg(String);
#[derive(Clone)]
struct TeamScore(u64);
#[derive(Clone, Default)]
struct TeamMembers(Vec<UserSearchItem>);

#[component]
pub fn NewGame() -> Element {
    let mut error_msg = use_context_provider(|| Signal::new(ErrorMsg::default()));
    let nav = use_navigator();

    let left_score = use_signal(|| TeamScore(13));
    let right_score = use_signal(|| TeamScore(0));
    let (left_title, right_title) = if left_score().0 >= right_score().0 {
        ("Équipe Gagnante", "Équipe Perdante")
    } else {
        ("Équipe Perdante", "Équipe Gagnante")
    };
    let left_team_members = use_signal(TeamMembers::default);
    let right_team_members = use_signal(TeamMembers::default);
    let mut submit_button_msg = use_signal(|| "Valider".to_string());
    let mut submit_loading = use_signal(|| false);
    let submit_game = move |_| {
        submit_loading.set(true);
        submit_button_msg.set("Chargement...".to_string());
        let game = GameSendItem::construct(
            left_score.read().0,
            right_score.read().0,
            &*left_team_members.read().0,
            &*right_team_members.read().0,
        );
        spawn(async move {
            match db::global::register_game(game).await {
                Ok(Ok(())) => {
                    submit_button_msg.set("Partie enregistrée !".to_string());
                    nav.push(Route::History);
                }
                Ok(Err(e)) => {
                    submit_button_msg.set("Valider".to_string());
                    error_msg.set(ErrorMsg(e));
                }
                Err(e) => {
                    submit_button_msg.set("Valider".to_string());
                    error_msg.set(ErrorMsg(e.to_string()));
                }
            }
            submit_loading.set(false)
        });
    };

    rsx! {
        document::Stylesheet { href : NEW_GAME_CSS }
        div { class: "new-game-container route-container",
            div { class: "team-entry-container",
                TeamEntry { title: left_title,
                     score: left_score,
                     members: left_team_members
                }
                TeamEntry {
                    title: right_title,
                    score: right_score,
                    members: right_team_members,
                }
            }
            { if !error_msg.read().0.is_empty() { rsx! {
                p { class: "error-message", {error_msg().0} }
            }} else { rsx! {} }}
            button { class: "validate-button",
                disabled: submit_loading(),
                onclick: submit_game,
                "{submit_button_msg}"
            }
        }
    }
}

#[component]
fn TeamEntry(title: String, score: Signal<TeamScore>, members: Signal<TeamMembers>) -> Element {
    provide_context(score);
    provide_context(members);
    rsx! {
        div { class: "team-entry",
            h2 { class: "team-entry-title",
                {title}
            }
            ScoreInput { }
            PlayerSelector {  }
        }
    }
}

#[component]
fn ScoreInput() -> Element {
    let mut score = use_context::<Signal<TeamScore>>();
    rsx! {
        div { class: "score-input",
            button { class: "score-input-buttons",
                onclick: move |_| if score().0 > 0 { score.set(TeamScore(score().0 - 1)) },
                img { class: "icon", src: MINUS_SVG}
            }
            input { class: "score-input-field",
                r#type: "text",
                inputmode: "numeric",
                pattern: "[0-9]*",
                placeholder: "0",
                value: "{score().0}",
                oninput: move |evt| {
                    let num = evt
                        .value()
                        .chars()
                        .filter(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<u64>()
                        .unwrap_or(0);
                    score.set(TeamScore(num.min(13)))
                },
            }
            button { class: "score-input-button",
                onclick: move |_| if score().0 < 13 { score.set(TeamScore(score().0 + 1)) },
                img { class: "icon", src: PLUS_SVG}
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
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(300).await;
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
                    img { class: "icon", src: CLOSE_SVG}

                }

            }

            {if !search_query.read().is_empty() { rsx! {
                div { class: "player-selector-search-box",
                    SuspenseBoundary {
                        fallback:|_| rsx! { p { class: "abnormal-state-message", "Patience..." } },
                        PlayerSearchBox {
                            search_query,
                            reset_query: move |()| search_query.set(String::new())
                        }
                    }
                }
            }} else { rsx!{} }}

            div { class: "selected-players-box",
                SelectedPlayersBox {  }
            }
        }
    }
}

#[component]
fn PlayerSearchBox(search_query: ReadSignal<String>, reset_query: EventHandler<()>) -> Element {
    let mut team_members = use_context::<Signal<TeamMembers>>();
    let cards_resource = use_server_future(move || {
        let s = search_query();
        async move { global::search_user(s).await }
    })?;
    let mut error_msg = use_context::<Signal<ErrorMsg>>();
    let on_player_click = move |item: UserSearchItem| {
        move |_| {
            if team_members.read().0.contains(&item) {
                error_msg.set(ErrorMsg(format!(
                    "{} ne peut pas se dupliquer",
                    item.username
                )));
            } else {
                team_members.write().0.push(item.clone());
                reset_query(());
            }
        }
    };

    match cards_resource() {
        Some(Ok(players)) if players.is_empty() => rsx! {
            p { class: "abnormal-state-message", "Aucun joueur trouvé"}
        },
        Some(Ok(players)) => rsx! {
            for item in players {
                { rsx!{
                    UserSearchCard {
                        on_click: on_player_click(item.clone()),
                        item
                    }
                }}
            }
        },
        Some(Err(e)) => {
            error_msg.set(ErrorMsg(e.to_string()));
            rsx! {
                p { class: "abnormal-state-message", "Erreur" }
            }
        }
        None => rsx! {
            p { class: "abnormal-state-message", "Patience..." }
        },
    }
}

#[component]
fn UserSearchCard(item: UserSearchItem, on_click: EventHandler<MouseEvent>) -> Element {
    rsx! {
        div { class: "user-search-card",
            onclick: on_click,
            div { class: "user-search-card-username",
                "{item.username}"
            }
            div { class: "user-search-card-elo cool-glow",
                "{item.elo}"
            }
        }
    }
}

#[component]
fn SelectedPlayersBox() -> Element {
    let mut team_members = use_context::<Signal<TeamMembers>>();
    if !team_members.read().0.is_empty() {
        rsx! {
            for (i, item) in team_members().0.iter().enumerate() { { rsx! {
                div { class: "selected-player",
                    UserSearchCard {
                        item: item.clone(),
                        on_click: move |_|()
                    }
                    button { class: "selected-player-remove-button",
                        onclick: move |_| _ = team_members.write().0.remove(i),
                        img { class: "icon", src: CLOSE_SVG}
                    }
                }
            }}}
        }
    } else {
        rsx! {
            p { class: "abnormal-state-message",
                "Équipe vide"
            }
        }
    }
}
