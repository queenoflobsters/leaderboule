//! Interface d'administration et de modération.
//!
//! Toutes les pages sont protégées côté client par `AdminOnly` et côté
//! serveur par `access::require_admin`. Le middleware HTTP ajoute une
//! troisième barrière pour les appels directs aux server functions.

use dioxus::{core::Task, prelude::*};

use crate::{
    api::{
        db::{current_user, global},
        moderation::{
            self, ActivityEntry, GameEditInput, ModerationGamePlayer, ModerationGameSummary,
            ModerationUserSummary, SanctionInput, UserStatsUpdate,
        },
        types::{SanctionKind, UserRole, UserStatus},
        utils,
    },
    client::{components::page_switcher::PageSwitcher, page_not_found::PageNotFound, route::Route},
};

const SHIELD_SVG: Asset = asset!("assets/icons/shield.svg");
const CLOSE_SVG: Asset = asset!("assets/icons/close.svg");
const RELOAD_SVG: Asset = asset!("assets/icons/reload.svg");

/// Garde de route côté client : un non-admin voit la 404.
#[component]
fn AdminOnly(children: Element) -> Element {
    let access = use_server_future(current_user::get_access)?;

    match access() {
        Some(Ok(info)) if info.is_admin => rsx! { {children} },
        Some(_) => rsx! { PageNotFound { segments: vec![] } },
        None => rsx! {
            p { class: "abnormal-state-message", "Vérification des droits..." }
        },
    }
}

fn status_class(status: UserStatus) -> &'static str {
    match status {
        UserStatus::Active => "badge status-active",
        UserStatus::Suspended => "badge status-suspended",
        UserStatus::Banned => "badge status-banned",
    }
}

fn role_class(role: UserRole) -> &'static str {
    match role {
        UserRole::Admin => "badge role-admin",
        UserRole::User => "badge role-user",
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Page principale : liste et recherche des utilisateurs.
#[component]
pub fn Moderation() -> Element {
    rsx! { AdminOnly { ModerationDashboard {} } }
}

#[component]
fn ModerationDashboard() -> Element {
    // La valeur affichée dans l'input est immédiate ; la requête serveur,
    // elle, est déclenchée après le debounce. Cela évite de faire suspendre
    // tout le tableau de bord (et donc de perdre le champ de recherche).
    let mut search_input = use_signal(String::new);
    let mut search_query = use_signal(String::new);
    let mut debounce_task = use_signal(|| None::<Task>);
    let mut status_filter = use_signal(|| Option::<UserStatus>::None);
    let mut current_page = use_signal(|| 0u64);
    let mut refresh = use_signal(|| 0u64);

    let on_search = move |evt: Event<FormData>| {
        let value = evt.value();
        search_input.set(value.clone());
        if let Some(task) = debounce_task.take() {
            task.cancel();
        }
        let task = spawn(async move {
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(300).await;
            current_page.set(0);
            search_query.set(value);
        });
        debounce_task.set(Some(task));
    };

    rsx! {
        div { class: "moderation-container route-container",
            header { class: "moderation-header",
                div {
                    h1 { class: "moderation-title",
                        img { class: "icon moderation-title-icon", src: SHIELD_SVG, alt: "" }
                        "Modération"
                    }
                    p { class: "moderation-subtitle",
                        "Inspectez les comptes, corrigez les statistiques et gérez les parties."
                    }
                }
                div { class: "moderation-header-actions",
                    Link { class: "important-button", to: Route::ModerationGames {}, "Voir les parties" }
                    button {
                        class: "moderation-icon-button",
                        r#type: "button",
                        aria_label: "Rafraîchir",
                        onclick: move |_| refresh += 1,
                        img { class: "icon", src: RELOAD_SVG, alt: "" }
                    }
                }
            }

            div { class: "moderation-toolbar",
                input {
                    class: "search-input moderation-search",
                    r#type: "search",
                    placeholder: "Rechercher par nom ou email...",
                    aria_label: "Rechercher un utilisateur",
                    value: "{search_input}",
                    oninput: on_search,
                }
                div { class: "moderation-filters",
                    button {
                        class: if status_filter().is_none() { "filter-button active" } else { "filter-button" },
                        r#type: "button",
                        onclick: move |_| { current_page.set(0); status_filter.set(None); },
                        "Tous"
                    }
                    button {
                        class: if status_filter() == Some(UserStatus::Active) { "filter-button active" } else { "filter-button" },
                        r#type: "button",
                        onclick: move |_| { current_page.set(0); status_filter.set(Some(UserStatus::Active)); },
                        "Actifs"
                    }
                    button {
                        class: if status_filter() == Some(UserStatus::Suspended) { "filter-button active" } else { "filter-button" },
                        r#type: "button",
                        onclick: move |_| { current_page.set(0); status_filter.set(Some(UserStatus::Suspended)); },
                        "Suspendus"
                    }
                    button {
                        class: if status_filter() == Some(UserStatus::Banned) { "filter-button active" } else { "filter-button" },
                        r#type: "button",
                        onclick: move |_| { current_page.set(0); status_filter.set(Some(UserStatus::Banned)); },
                        "Bannis"
                    }
                }
            }

            SuspenseBoundary {
                fallback: |_| rsx! { p { class: "abnormal-state-message", "Patience..." } },
                UserList {
                    search_query,
                    status_filter,
                    current_page,
                    refresh,
                    on_page_change: move |value| current_page.set(value),
                }
            }

            SuspenseBoundary {
                fallback: |_| rsx! {},
                RecentAudit { refresh }
            }
        }
    }
}

#[component]
fn UserList(
    search_query: ReadSignal<String>,
    status_filter: ReadSignal<Option<UserStatus>>,
    current_page: ReadSignal<u64>,
    refresh: ReadSignal<u64>,
    on_page_change: EventHandler<u64>,
) -> Element {
    let users_resource = use_server_future(move || {
        let _ = refresh();
        let query = search_query();
        let status = status_filter();
        let page = current_page();
        async move { moderation::moderation_list_users(query, status, page).await }
    })?;

    match users_resource() {
        Some(Ok(user_page)) => rsx! {
            p { class: "moderation-count", "{user_page.total} compte(s)" }
            div { class: "moderation-user-list",
                for user in user_page.users.iter() {
                    UserRow { key: "{user.id}", user: user.clone() }
                }
            }
            if user_page.users.is_empty() {
                p { class: "abnormal-state-message", "Aucun utilisateur trouvé" }
            }
            if user_page.total > 20 {
                PageSwitcher {
                    current_page: user_page.page,
                    current_page_change: move |value| on_page_change.call(value),
                }
            }
        },
        Some(Err(error)) => rsx! {
            p { class: "abnormal-state-message", "Erreur : {error}" }
        },
        None => rsx! {
            p { class: "abnormal-state-message", "Chargement..." }
        },
    }
}

#[component]
fn UserRow(user: ModerationUserSummary) -> Element {
    let member_since = utils::format_date(user.member_since);
    rsx! {
        Link {
            class: "moderation-user-row",
            to: Route::ModerationUser { user_id: user.id.clone() },
            div { class: "moderation-user-main",
                div { class: "moderation-user-name-line",
                    span { class: "moderation-username", "{user.username}" }
                    span { class: role_class(user.role), "{user.role.label_fr()}" }
                    span { class: status_class(user.status), "{user.status.label_fr()}" }
                    if user.is_env_admin {
                        span { class: "badge env-admin", "Admin .env" }
                    }
                }
                span { class: "moderation-email", "{user.email}" }
                span { class: "moderation-meta", "Membre depuis le {member_since}" }
            }
            div { class: "moderation-user-stats",
                span { class: "moderation-stat", span { "Elo" } strong { "{user.elo}" } }
                span { class: "moderation-stat", span { "Parties" } strong { "{user.games_played}" } }
                span { class: "moderation-stat", span { "Victoires" } strong { "{user.games_won}" } }
            }
        }
    }
}

#[component]
fn RecentAudit(refresh: ReadSignal<u64>) -> Element {
    let audit_resource = use_server_future(move || {
        let _ = refresh();
        async move { moderation::moderation_recent_audit(0).await }
    })?;

    match audit_resource() {
        Some(Ok(entries)) if entries.is_empty() => rsx! {},
        Some(Ok(entries)) => rsx! {
            section { class: "moderation-panel moderation-audit-panel",
                h2 { "Dernières actions de modération" }
                div { class: "moderation-timeline compact",
                    for entry in entries.iter() {
                        div { class: "timeline-entry", key: "{entry.id}",
                            span { class: "timeline-dot moderation" }
                            div { class: "timeline-content",
                                span { class: "timeline-label", "{entry.action}" }
                                span { class: "timeline-detail",
                                    { match &entry.target_username {
                                        Some(target) => format!("{} → {} · {}", entry.actor_username, target, entry.details),
                                        None => format!("{} · {}", entry.actor_username, entry.details),
                                    } }
                                }
                                span { class: "timeline-date", "{utils::format_date_and_hour(entry.created_at)}" }
                            }
                        }
                    }
                }
            }
        },
        _ => rsx! {},
    }
}

/// Inspecteur détaillé d'un utilisateur.
#[component]
pub fn ModerationUser(user_id: String) -> Element {
    rsx! { AdminOnly { UserInspector { user_id } } }
}

#[component]
fn UserInspector(user_id: String) -> Element {
    let refresh = use_signal(|| 0u64);
    let id_for_future = user_id.clone();

    let detail_resource = use_server_future(move || {
        let _ = refresh();
        let id = id_for_future.clone();
        async move { moderation::moderation_get_user(id).await }
    })?;

    match detail_resource() {
        Some(Ok(detail)) => rsx! {
            div { class: "moderation-container moderation-detail route-container",
                Link { class: "back-link", to: Route::Moderation {}, "← Retour à la modération" }

                header { class: "moderation-detail-header",
                    div { class: "moderation-identity",
                        h1 { "{detail.user.username}" }
                        div { class: "moderation-badges",
                            span { class: role_class(detail.user.role), "{detail.user.role.label_fr()}" }
                            span { class: status_class(detail.user.status), "{detail.user.status.label_fr()}" }
                            if detail.user.is_env_admin {
                                span { class: "badge env-admin", "Admin .env" }
                            }
                        }
                        p { class: "moderation-email", "{detail.user.email}" }
                        p { class: "moderation-meta",
                            "Membre depuis le {utils::format_date(detail.user.member_since)}"
                        }
                        if let Some(actor) = &detail.moderated_by {
                            p { class: "moderation-meta", "Dernière modération par {actor}" }
                        }
                    }
                    div { class: "moderation-detail-stats",
                        span { class: "moderation-stat", span { "Elo" } strong { "{detail.user.elo}" } }
                        span { class: "moderation-stat", span { "Record" } strong { "{detail.user.best_elo}" } }
                        span { class: "moderation-stat", span { "Parties" } strong { "{detail.user.games_played}" } }
                        span { class: "moderation-stat", span { "Victoires" } strong { "{detail.user.games_won}" } }
                        span { class: "moderation-stat", span { "Défaites" } strong { "{detail.user.games_lost}" } }
                        span { class: "moderation-stat", span { "Ratio" } strong { "{detail.user.win_ratio:.1}%" } }
                    }
                }

                div { class: "moderation-grid",
                    SanctionPanel { user: detail.user.clone(), refresh }
                    RolePanel { user: detail.user.clone(), refresh }
                    StatsEditor {
                        key: "{detail.user.id}-{detail.user.elo}-{detail.user.best_elo}-{detail.user.games_played}-{detail.user.games_won}",
                        user: detail.user.clone(),
                        refresh,
                    }
                    UsernameEditor {
                        key: "{detail.user.id}-{detail.user.username}",
                        user: detail.user.clone(),
                        refresh,
                    }
                    PasswordResetPanel {
                        user: detail.user.clone(),
                        has_password: detail.has_password,
                        is_self: detail.is_self,
                        refresh,
                    }
                }

                section { class: "moderation-panel",
                    h2 { "Historique d'activité" }
                    ActivityTimeline { entries: detail.activity.clone() }
                }

                section { class: "moderation-panel",
                    div { class: "moderation-panel-heading",
                        h2 { "Parties récentes" }
                        Link { class: "important-button", to: Route::ModerationGames {}, "Toutes les parties" }
                    }
                    if detail.games.is_empty() {
                        p { class: "abnormal-state-message", "Aucune partie" }
                    } else {
                        div { class: "moderation-game-list",
                            for game in detail.games.iter() {
                                GameAdminCard { key: "{game.id}", game: game.clone(), refresh }
                            }
                        }
                    }
                }
            }
        },
        Some(Err(error)) => rsx! {
            div { class: "moderation-container route-container",
                Link { class: "back-link", to: Route::Moderation {}, "← Retour" }
                p { class: "abnormal-state-message", "Erreur : {error}" }
            }
        },
        None => rsx! {
            p { class: "abnormal-state-message", "Chargement de l'inspecteur..." }
        },
    }
}

fn parse_duration(value: &str) -> Option<u64> {
    match value {
        "" => None,
        value => value.parse::<u64>().ok(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_sanction_request(
    user_id: String,
    kind: SanctionKind,
    reason_value: String,
    duration_hours: Option<u64>,
    mut loading: Signal<bool>,
    mut feedback: Signal<Option<(bool, String)>>,
    mut refresh: Signal<u64>,
    mut reason_signal: Signal<String>,
) {
    let sanction = SanctionInput {
        kind,
        reason: reason_value,
        duration_hours: if matches!(kind, SanctionKind::Suspend) {
            duration_hours
        } else {
            None
        },
    };

    match moderation::moderation_apply_sanction(user_id, sanction).await {
        Ok(Ok(())) => {
            reason_signal.set(String::new());
            feedback.set(Some((
                true,
                "Sanction appliquée. Les sessions ont été fermées.".to_string(),
            )));
            refresh += 1;
        }
        Ok(Err(error)) => feedback.set(Some((false, error))),
        Err(error) => feedback.set(Some((false, error.to_string()))),
    }
    loading.set(false);
}

#[component]
fn SanctionPanel(user: ModerationUserSummary, refresh: Signal<u64>) -> Element {
    let mut reason = use_signal(String::new);
    let mut duration = use_signal(|| "24".to_string());
    let mut loading = use_signal(|| false);
    let mut feedback = use_signal(|| Option::<(bool, String)>::None);
    let user_id = user.id.clone();

    let submit_suspend = {
        let user_id = user_id.clone();
        move |_| {
            let user_id = user_id.clone();
            let reason_value = reason();
            let duration_value = parse_duration(&duration());
            loading.set(true);
            feedback.set(None);
            spawn(async move {
                apply_sanction_request(
                    user_id,
                    SanctionKind::Suspend,
                    reason_value,
                    duration_value,
                    loading,
                    feedback,
                    refresh,
                    reason,
                )
                .await;
            });
        }
    };
    let submit_ban = {
        let user_id = user.id.clone();
        move |_| {
            let user_id = user_id.clone();
            let reason_value = reason();
            loading.set(true);
            feedback.set(None);
            spawn(async move {
                apply_sanction_request(
                    user_id,
                    SanctionKind::Ban,
                    reason_value,
                    None,
                    loading,
                    feedback,
                    refresh,
                    reason,
                )
                .await;
            });
        }
    };

    let lift = {
        let user_id = user.id.clone();
        move |_| {
            let user_id = user_id.clone();
            loading.set(true);
            feedback.set(None);
            spawn(async move {
                match moderation::moderation_lift_sanction(user_id).await {
                    Ok(Ok(())) => {
                        feedback.set(Some((true, "Sanction levée.".to_string())));
                        refresh += 1;
                    }
                    Ok(Err(error)) => feedback.set(Some((false, error))),
                    Err(error) => feedback.set(Some((false, error.to_string()))),
                }
                loading.set(false);
            });
        }
    };

    rsx! {
        section { class: "moderation-panel",
            h2 { "Sanction" }
            div { class: "sanction-state",
                span { class: status_class(user.status), "{user.status.label_fr()}" }
                if let Some(until) = user.suspended_until {
                    span { class: "moderation-meta",
                        "Jusqu'au {utils::format_date_and_hour(until)}"
                    }
                }
            }
            if let Some(reason_text) = &user.status_reason {
                p { class: "sanction-reason", "Motif : {reason_text}" }
            }

            label { class: "moderation-label", "Motif" }
            textarea {
                class: "moderation-textarea",
                placeholder: "Expliquez la raison de la sanction...",
                value: "{reason}",
                oninput: move |evt| reason.set(evt.value()),
            }

            label { class: "moderation-label", "Durée de suspension" }
            select {
                class: "moderation-select",
                value: "{duration}",
                onchange: move |evt| duration.set(evt.value()),
                option { value: "", "Indéterminée" }
                option { value: "1", "1 heure" }
                option { value: "24", "24 heures" }
                option { value: "168", "7 jours" }
                option { value: "720", "30 jours" }
                option { value: "8760", "1 an" }
            }

            Feedback { message: feedback() }

            div { class: "moderation-button-row",
                button {
                    class: "important-button warning-button",
                    r#type: "button",
                    disabled: loading(),
                    onclick: submit_suspend,
                    "Suspendre"
                }
                button {
                    class: "important-button danger-button",
                    r#type: "button",
                    disabled: loading(),
                    onclick: submit_ban,
                    "Bannir"
                }
                if !user.status.is_active() {
                    button {
                        class: "important-button",
                        r#type: "button",
                        disabled: loading(),
                        onclick: lift,
                        "Lever la sanction"
                    }
                }
            }
        }
    }
}

#[component]
fn RolePanel(user: ModerationUserSummary, refresh: Signal<u64>) -> Element {
    let mut loading = use_signal(|| false);
    let mut confirm = use_signal(|| false);
    let mut feedback = use_signal(|| Option::<(bool, String)>::None);
    let is_admin_role = user.role.is_admin();
    let can_change = !user.is_env_admin;
    let user_id = user.id.clone();

    let toggle = move |_| {
        if is_admin_role && !confirm() {
            confirm.set(true);
            return;
        }

        let user_id = user_id.clone();
        let new_role = if is_admin_role {
            UserRole::User
        } else {
            UserRole::Admin
        };
        loading.set(true);
        feedback.set(None);
        spawn(async move {
            match moderation::moderation_set_role(user_id, new_role).await {
                Ok(Ok(())) => {
                    feedback.set(Some((true, "Rôle mis à jour.".to_string())));
                    confirm.set(false);
                    refresh += 1;
                }
                Ok(Err(error)) => feedback.set(Some((false, error))),
                Err(error) => feedback.set(Some((false, error.to_string()))),
            }
            loading.set(false);
        });
    };

    rsx! {
        section { class: "moderation-panel",
            h2 { "Rôle" }
            div { class: "sanction-state",
                span { class: role_class(user.role), "{user.role.label_fr()}" }
                if user.is_env_admin {
                    span { class: "badge env-admin", "ADMIN_EMAILS" }
                }
            }
            if user.is_env_admin {
                p { class: "moderation-hint",
                    "Ce compte est toujours administrateur via la variable ADMIN_EMAILS."
                }
            } else {
                p { class: "moderation-hint",
                    if is_admin_role {
                        "Une rétrogradation retire immédiatement l'accès à la modération."
                    } else {
                        "Un administrateur peut inspecter tous les comptes et modifier les parties."
                    }
                }
            }
            Feedback { message: feedback() }
            button {
                class: if is_admin_role { "important-button danger-button" } else { "important-button" },
                r#type: "button",
                disabled: loading() || !can_change,
                onclick: toggle,
                { if is_admin_role {
                    if confirm() { "Confirmer la rétrogradation" } else { "Rétrograder en utilisateur" }
                } else {
                    "Promouvoir administrateur"
                } }
            }
        }
    }
}

#[component]
fn StatsEditor(user: ModerationUserSummary, refresh: Signal<u64>) -> Element {
    let mut elo = use_signal(|| user.elo);
    let mut best_elo = use_signal(|| user.best_elo);
    let mut games_played = use_signal(|| user.games_played);
    let mut games_won = use_signal(|| user.games_won);
    let mut loading = use_signal(|| false);
    let mut feedback = use_signal(|| Option::<(bool, String)>::None);
    let user_id = user.id.clone();

    let derived_lost = (games_played() - games_won()).max(0);
    let derived_ratio = if games_played() > 0 {
        games_won() as f32 * 100.0 / games_played() as f32
    } else {
        0.0
    };

    let save = move |_| {
        if elo() < 0 {
            feedback.set(Some((false, "L'Elo ne peut pas être négatif.".to_string())));
            return;
        }
        if games_played() < 0 {
            feedback.set(Some((
                false,
                "Le nombre de parties ne peut pas être négatif.".to_string(),
            )));
            return;
        }
        if games_won() < 0 || games_won() > games_played() {
            feedback.set(Some((
                false,
                "Les victoires doivent être comprises entre 0 et le nombre de parties jouées."
                    .to_string(),
            )));
            return;
        }
        if best_elo() < elo() {
            feedback.set(Some((
                false,
                "Le record d'Elo doit être supérieur ou égal à l'Elo actuel.".to_string(),
            )));
            return;
        }

        let user_id = user_id.clone();
        let update = UserStatsUpdate {
            elo: elo(),
            best_elo: best_elo(),
            games_played: games_played(),
            games_won: games_won(),
        };
        loading.set(true);
        feedback.set(None);
        spawn(async move {
            match moderation::moderation_update_stats(user_id, update).await {
                Ok(Ok(())) => {
                    feedback.set(Some((true, "Statistiques enregistrées.".to_string())));
                    refresh += 1;
                }
                Ok(Err(error)) => feedback.set(Some((false, error))),
                Err(error) => feedback.set(Some((false, error.to_string()))),
            }
            loading.set(false);
        });
    };

    rsx! {
        section { class: "moderation-panel",
            h2 { "Statistiques" }
            div { class: "moderation-form-grid",
                label { class: "moderation-field",
                    span { "Elo" }
                    input {
                        class: "important-input",
                        r#type: "number",
                        min: "0",
                        value: "{elo}",
                        oninput: move |evt| elo.set(evt.value().parse::<i64>().unwrap_or(0)),
                    }
                }
                label { class: "moderation-field",
                    span { "Record d'Elo" }
                    input {
                        class: "important-input",
                        r#type: "number",
                        min: "0",
                        value: "{best_elo}",
                        oninput: move |evt| best_elo.set(evt.value().parse::<i64>().unwrap_or(0)),
                    }
                }
                label { class: "moderation-field",
                    span { "Parties jouées" }
                    input {
                        class: "important-input",
                        r#type: "number",
                        min: "0",
                        value: "{games_played}",
                        oninput: move |evt| games_played.set(evt.value().parse::<i64>().unwrap_or(0)),
                    }
                }
                label { class: "moderation-field",
                    span { "Victoires" }
                    input {
                        class: "important-input",
                        r#type: "number",
                        min: "0",
                        value: "{games_won}",
                        oninput: move |evt| games_won.set(evt.value().parse::<i64>().unwrap_or(0)),
                    }
                }
            }
            p { class: "moderation-derived",
                "Défaites calculées : {derived_lost} · Ratio : {derived_ratio:.1}%"
            }
            Feedback { message: feedback() }
            button {
                class: "important-button",
                r#type: "button",
                disabled: loading(),
                onclick: save,
                "Enregistrer les statistiques"
            }
        }
    }
}

#[component]
fn UsernameEditor(user: ModerationUserSummary, refresh: Signal<u64>) -> Element {
    let mut value = use_signal(|| user.username.clone());
    let mut loading = use_signal(|| false);
    let mut feedback = use_signal(|| Option::<(bool, String)>::None);
    let user_id = user.id.clone();
    let sanitized = moderation::sanitize_username(&value());
    let show_preview = !sanitized.is_empty() && sanitized != value();

    let sanitize = {
        let sanitized = sanitized.clone();
        move |_| value.set(sanitized.clone())
    };

    let save = move |_| {
        let user_id = user_id.clone();
        let new_username = value();
        if new_username.trim().is_empty() {
            feedback.set(Some((false, "Le nom d'utilisateur est vide.".to_string())));
            return;
        }
        loading.set(true);
        feedback.set(None);
        spawn(async move {
            match moderation::moderation_change_username(user_id, new_username).await {
                Ok(Ok(applied)) => {
                    value.set(applied);
                    feedback.set(Some((true, "Nom d'utilisateur mis à jour.".to_string())));
                    refresh += 1;
                }
                Ok(Err(error)) => feedback.set(Some((false, error))),
                Err(error) => feedback.set(Some((false, error.to_string()))),
            }
            loading.set(false);
        });
    };

    rsx! {
        section { class: "moderation-panel",
            h2 { "Nom d'utilisateur" }
            label { class: "moderation-field",
                span { "Nom actuel : {user.username}" }
                input {
                    class: "important-input",
                    r#type: "text",
                    maxlength: "32",
                    value: "{value}",
                    oninput: move |evt| value.set(evt.value()),
                }
            }
            if show_preview {
                p { class: "moderation-hint",
                    "Version assainie : « {sanitized} »"
                }
            }
            Feedback { message: feedback() }
            div { class: "moderation-button-row",
                button {
                    class: "important-button",
                    r#type: "button",
                    disabled: loading() || !show_preview,
                    onclick: sanitize,
                    "Assainir"
                }
                button {
                    class: "important-button",
                    r#type: "button",
                    disabled: loading(),
                    onclick: save,
                    "Forcer le changement"
                }
            }
        }
    }
}

#[component]
fn PasswordResetPanel(
    user: ModerationUserSummary,
    has_password: bool,
    is_self: bool,
    refresh: Signal<u64>,
) -> Element {
    let nav = use_navigator();
    let mut confirm = use_signal(|| false);
    let mut loading = use_signal(|| false);
    let mut feedback = use_signal(|| Option::<(bool, String)>::None);
    let user_id = user.id.clone();

    let reset = move |_| {
        if !confirm() {
            confirm.set(true);
            return;
        }

        let user_id = user_id.clone();
        loading.set(true);
        feedback.set(None);
        spawn(async move {
            match moderation::moderation_reset_password(user_id).await {
                Ok(Ok(())) => {
                    if is_self {
                        nav.push(Route::Login);
                    } else {
                        feedback.set(Some((
                            true,
                            "Mot de passe réinitialisé. Le prochain mot de passe saisi par l'utilisateur à la connexion deviendra le nouveau.".to_string(),
                        )));
                        refresh += 1;
                    }
                }
                Ok(Err(error)) => feedback.set(Some((false, error))),
                Err(error) => feedback.set(Some((false, error.to_string()))),
            }
            loading.set(false);
            confirm.set(false);
        });
    };

    rsx! {
        section { class: "moderation-panel",
            h2 { "Mot de passe" }
            div { class: "sanction-state",
                span {
                    class: if has_password { "badge status-active" } else { "badge status-suspended" },
                    { if has_password { "Défini" } else { "Non défini" } }
                }
            }
            p { class: "moderation-hint",
                { if has_password {
                    "Réinitialiser le mot de passe supprimera l'ancien. À sa prochaine connexion, l'utilisateur devra saisir un nouveau mot de passe : celui qu'il entrera deviendra son mot de passe."
                } else {
                    "Aucun mot de passe n'est défini. Le prochain mot de passe saisi à la connexion sera enregistré."
                } }
            }
            if is_self {
                p { class: "moderation-hint",
                    "Vous réinitialisez votre propre mot de passe : vous serez déconnecté après l'opération."
                }
            }
            Feedback { message: feedback() }
            button {
                class: if confirm() {
                    "important-button danger-button"
                } else {
                    "important-button warning-button"
                },
                r#type: "button",
                disabled: loading(),
                onclick: reset,
                { if confirm() { "Confirmer la réinitialisation" } else { "Réinitialiser le mot de passe" } }
            }
        }
    }
}

#[component]
fn Feedback(message: Option<(bool, String)>) -> Element {
    match message {
        Some((true, text)) => rsx! { p { class: "moderation-success", role: "status", "{text}" } },
        Some((false, text)) => rsx! { p { class: "form-error", role: "alert", "{text}" } },
        None => rsx! {},
    }
}

#[component]
fn ActivityTimeline(entries: Vec<ActivityEntry>) -> Element {
    if entries.is_empty() {
        return rsx! { p { class: "abnormal-state-message", "Aucune activité enregistrée" } };
    }

    rsx! {
        div { class: "moderation-timeline",
            for entry in entries.iter() {
                div { class: "timeline-entry", key: "{entry.occurred_at}-{entry.label}-{entry.detail}",
                    span { class: "timeline-dot {entry.kind}" }
                    div { class: "timeline-content",
                        span { class: "timeline-label", "{entry.label}" }
                        span { class: "timeline-detail", "{entry.detail}" }
                        span { class: "timeline-date", "{utils::format_date_and_hour(entry.occurred_at)}" }
                    }
                }
            }
        }
    }
}

#[component]
fn GameAdminCard(game: ModerationGameSummary, refresh: Signal<u64>) -> Element {
    let mut confirm_delete = use_signal(|| false);
    let mut loading = use_signal(|| false);
    let mut feedback = use_signal(|| Option::<(bool, String)>::None);
    let game_id = game.id.clone();
    let winners: Vec<&ModerationGamePlayer> =
        game.players.iter().filter(|player| player.won).collect();
    let losers: Vec<&ModerationGamePlayer> =
        game.players.iter().filter(|player| !player.won).collect();

    let delete = move |_| {
        if !confirm_delete() {
            confirm_delete.set(true);
            return;
        }
        let game_id = game_id.clone();
        loading.set(true);
        feedback.set(None);
        spawn(async move {
            match moderation::moderation_delete_game(game_id).await {
                Ok(Ok(())) => {
                    feedback.set(Some((
                        true,
                        "Partie supprimée et effets annulés.".to_string(),
                    )));
                    refresh += 1;
                }
                Ok(Err(error)) => feedback.set(Some((false, error))),
                Err(error) => feedback.set(Some((false, error.to_string()))),
            }
            loading.set(false);
            confirm_delete.set(false);
        });
    };

    rsx! {
        article { class: "moderation-game-card",
            div { class: "moderation-game-head",
                div {
                    span { class: "moderation-game-score", "{game.won_score} - {game.lost_score}" }
                    span { class: "moderation-meta",
                        "{utils::format_date_and_hour(game.played_at)} · par {game.recorded_by}"
                    }
                    span { class: "moderation-game-id", "#{short_id(&game.id)}" }
                }
                div { class: "moderation-game-actions",
                    Link {
                        class: "important-button",
                        to: Route::ModerationGame { game_id: game.id.clone() },
                        "Modifier"
                    }
                    button {
                        class: if confirm_delete() {
                            "important-button danger-button"
                        } else {
                            "important-button warning-button"
                        },
                        r#type: "button",
                        disabled: loading(),
                        onclick: delete,
                        { if confirm_delete() { "Confirmer" } else { "Supprimer" } }
                    }
                }
            }
            div { class: "moderation-game-teams",
                div { class: "moderation-team won",
                    span { class: "moderation-team-title", "Gagnants" }
                    div { class: "moderation-chips",
                        for player in winners.iter() {
                            span { class: "moderation-chip won", key: "{player.user_id}",
                                "{player.username}"
                                span { class: "chip-elo", "{player.elo_change:+}" }
                            }
                        }
                    }
                }
                div { class: "moderation-team lost",
                    span { class: "moderation-team-title", "Perdants" }
                    div { class: "moderation-chips",
                        for player in losers.iter() {
                            span { class: "moderation-chip lost", key: "{player.user_id}",
                                "{player.username}"
                                span { class: "chip-elo", "{player.elo_change:+}" }
                            }
                        }
                    }
                }
            }
            Feedback { message: feedback() }
        }
    }
}

/// Liste globale des parties.
#[component]
pub fn ModerationGames() -> Element {
    rsx! { AdminOnly { GamesDashboard {} } }
}

#[component]
fn GamesDashboard() -> Element {
    let mut search_input = use_signal(String::new);
    let mut search_query = use_signal(String::new);
    let mut debounce_task = use_signal(|| None::<Task>);
    let mut current_page = use_signal(|| 0u64);
    let mut refresh = use_signal(|| 0u64);

    let on_search = move |evt: Event<FormData>| {
        let value = evt.value();
        search_input.set(value.clone());
        if let Some(task) = debounce_task.take() {
            task.cancel();
        }
        let task = spawn(async move {
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(300).await;
            current_page.set(0);
            search_query.set(value);
        });
        debounce_task.set(Some(task));
    };

    rsx! {
        div { class: "moderation-container route-container",
            Link { class: "back-link", to: Route::Moderation {}, "← Retour à la modération" }
            header { class: "moderation-header",
                div {
                    h1 { class: "moderation-title", "Parties" }
                    p { class: "moderation-subtitle",
                        "Recherchez une partie par joueur ou identifiant, puis corrigez-la."
                    }
                }
                button {
                    class: "moderation-icon-button",
                    r#type: "button",
                    aria_label: "Rafraîchir",
                    onclick: move |_| refresh += 1,
                    img { class: "icon", src: RELOAD_SVG, alt: "" }
                }
            }

            input {
                class: "search-input moderation-search",
                r#type: "search",
                placeholder: "Rechercher un joueur ou une partie...",
                aria_label: "Rechercher une partie",
                value: "{search_input}",
                oninput: on_search,
            }

            SuspenseBoundary {
                fallback: |_| rsx! { p { class: "abnormal-state-message", "Patience..." } },
                GameList {
                    search_query,
                    current_page,
                    refresh,
                    on_page_change: move |value| current_page.set(value),
                }
            }
        }
    }
}

#[component]
fn GameList(
    search_query: ReadSignal<String>,
    current_page: ReadSignal<u64>,
    refresh: Signal<u64>,
    on_page_change: EventHandler<u64>,
) -> Element {
    let games_resource = use_server_future(move || {
        let _ = refresh();
        let query = search_query();
        let page = current_page();
        async move { moderation::moderation_list_games(query, page).await }
    })?;

    match games_resource() {
        Some(Ok(game_page)) => rsx! {
            p { class: "moderation-count", "{game_page.total} partie(s)" }
            div { class: "moderation-game-list",
                for game in game_page.games.iter() {
                    GameAdminCard { key: "{game.id}", game: game.clone(), refresh }
                }
            }
            if game_page.games.is_empty() {
                p { class: "abnormal-state-message", "Aucune partie trouvée" }
            }
            if game_page.total > 12 {
                PageSwitcher {
                    current_page: game_page.page,
                    current_page_change: move |value| on_page_change.call(value),
                }
            }
        },
        Some(Err(error)) => rsx! {
            p { class: "abnormal-state-message", "Erreur : {error}" }
        },
        None => rsx! {
            p { class: "abnormal-state-message", "Chargement..." }
        },
    }
}

/// Éditeur d'une partie.
#[component]
pub fn ModerationGamePage(game_id: String) -> Element {
    rsx! { AdminOnly { GameEditor { game_id } } }
}

#[component]
fn GameEditor(game_id: String) -> Element {
    let id_for_future = game_id.clone();
    let game_resource = use_server_future(move || {
        let id = id_for_future.clone();
        async move { moderation::moderation_get_game(id).await }
    })?;

    match game_resource() {
        Some(Ok(game)) => rsx! {
            GameEditorForm { key: "{game.id}", game }
        },
        Some(Err(error)) => rsx! {
            div { class: "moderation-container route-container",
                Link { class: "back-link", to: Route::ModerationGames {}, "← Retour aux parties" }
                p { class: "abnormal-state-message", "Erreur : {error}" }
            }
        },
        None => rsx! {
            p { class: "abnormal-state-message", "Chargement de la partie..." }
        },
    }
}

#[component]
fn GameEditorForm(game: ModerationGameSummary) -> Element {
    let nav = use_navigator();
    let winning_initial: Vec<String> = game
        .players
        .iter()
        .filter(|player| player.won)
        .map(|player| player.username.clone())
        .collect();
    let losing_initial: Vec<String> = game
        .players
        .iter()
        .filter(|player| !player.won)
        .map(|player| player.username.clone())
        .collect();

    let mut won_score = use_signal(|| game.won_score.max(0) as u64);
    let mut lost_score = use_signal(|| game.lost_score.max(0) as u64);
    let winning_team = use_signal(move || winning_initial.clone());
    let losing_team = use_signal(move || losing_initial.clone());
    let mut loading = use_signal(|| false);
    let mut confirm_delete = use_signal(|| false);
    let mut feedback = use_signal(|| Option::<(bool, String)>::None);
    let game_id = game.id.clone();

    let save = {
        let game_id = game_id.clone();
        move |_| {
            if won_score() > 13 || lost_score() > 13 {
                feedback.set(Some((false, "Le score maximum est de 13.".to_string())));
                return;
            }
            if won_score() <= lost_score() {
                feedback.set(Some((
                    false,
                    "Le score gagnant doit être strictement supérieur au score perdant."
                        .to_string(),
                )));
                return;
            }
            if won_score() < 3 {
                feedback.set(Some((
                    false,
                    "Le score gagnant doit être au moins 3.".to_string(),
                )));
                return;
            }
            if winning_team().is_empty() || losing_team().is_empty() {
                feedback.set(Some((
                    false,
                    "Les deux équipes doivent contenir au moins un joueur.".to_string(),
                )));
                return;
            }

            let input = GameEditInput {
                game_id: game_id.clone(),
                won_score: won_score(),
                lost_score: lost_score(),
                winning_team: winning_team(),
                losing_team: losing_team(),
            };
            loading.set(true);
            feedback.set(None);
            spawn(async move {
                match moderation::moderation_update_game(input).await {
                    Ok(Ok(())) => {
                        nav.push(Route::ModerationGames);
                    }
                    Ok(Err(error)) => feedback.set(Some((false, error))),
                    Err(error) => feedback.set(Some((false, error.to_string()))),
                }
                loading.set(false);
            });
        }
    };

    let delete = {
        let game_id = game_id.clone();
        move |_| {
            if !confirm_delete() {
                confirm_delete.set(true);
                return;
            }
            let game_id = game_id.clone();
            loading.set(true);
            feedback.set(None);
            spawn(async move {
                match moderation::moderation_delete_game(game_id).await {
                    Ok(Ok(())) => {
                        nav.push(Route::ModerationGames);
                    }
                    Ok(Err(error)) => feedback.set(Some((false, error))),
                    Err(error) => feedback.set(Some((false, error.to_string()))),
                }
                loading.set(false);
            });
        }
    };

    rsx! {
        div { class: "moderation-container route-container",
            Link { class: "back-link", to: Route::ModerationGames {}, "← Retour aux parties" }
            header { class: "moderation-header",
                div {
                    h1 { class: "moderation-title", "Éditer la partie" }
                    p { class: "moderation-subtitle",
                        "Partie #{short_id(&game.id)} · enregistrée le {utils::format_date_and_hour(game.played_at)}"
                    }
                }
            }

            div { class: "moderation-warning",
                "Modifier cette partie annule d'abord son ancien Elo pour tous les joueurs, puis applique le nouveau calcul. Les stats gagnées/perdues sont recalculées."
            }

            div { class: "moderation-game-editor",
                div { class: "moderation-score-row",
                    div { class: "moderation-score-block",
                        label { class: "moderation-label", "Score gagnant" }
                        div { class: "moderation-score-controls",
                            button {
                                class: "score-input-button",
                                r#type: "button",
                                onclick: move |_| won_score.set(won_score().saturating_sub(1)),
                                "−"
                            }
                            input {
                                class: "score-input-field",
                                r#type: "number",
                                min: "3",
                                max: "13",
                                value: "{won_score}",
                                oninput: move |evt| won_score.set(evt.value().parse::<u64>().unwrap_or(0).min(13)),
                            }
                            button {
                                class: "score-input-button",
                                r#type: "button",
                                onclick: move |_| won_score.set((won_score() + 1).min(13)),
                                "+"
                            }
                        }
                    }
                    div { class: "moderation-score-block",
                        label { class: "moderation-label", "Score perdant" }
                        div { class: "moderation-score-controls",
                            button {
                                class: "score-input-button",
                                r#type: "button",
                                onclick: move |_| lost_score.set(lost_score().saturating_sub(1)),
                                "−"
                            }
                            input {
                                class: "score-input-field",
                                r#type: "number",
                                min: "0",
                                max: "12",
                                value: "{lost_score}",
                                oninput: move |evt| lost_score.set(evt.value().parse::<u64>().unwrap_or(0).min(13)),
                            }
                            button {
                                class: "score-input-button",
                                r#type: "button",
                                onclick: move |_| lost_score.set((lost_score() + 1).min(13)),
                                "+"
                            }
                        }
                    }
                }

                TeamEditor {
                    title: "Équipe gagnante".to_string(),
                    own: winning_team,
                    other: losing_team,
                    accent: "won",
                }
                TeamEditor {
                    title: "Équipe perdante".to_string(),
                    own: losing_team,
                    other: winning_team,
                    accent: "lost",
                }
            }

            Feedback { message: feedback() }

            div { class: "moderation-button-row",
                button {
                    class: "important-button",
                    r#type: "button",
                    disabled: loading(),
                    onclick: save,
                    "Enregistrer la partie"
                }
                button {
                    class: if confirm_delete() { "important-button danger-button" } else { "important-button warning-button" },
                    r#type: "button",
                    disabled: loading(),
                    onclick: delete,
                    { if confirm_delete() { "Confirmer la suppression" } else { "Supprimer la partie" } }
                }
            }
        }
    }
}

#[component]
fn TeamEditor(
    title: String,
    own: Signal<Vec<String>>,
    other: Signal<Vec<String>>,
    accent: String,
) -> Element {
    let mut search_input = use_signal(String::new);
    let mut search_query = use_signal(String::new);
    let mut debounce_task = use_signal(|| None::<Task>);

    let on_input = move |evt: Event<FormData>| {
        let value = evt.value();
        search_input.set(value.clone());
        if let Some(task) = debounce_task.take() {
            task.cancel();
        }
        let task = spawn(async move {
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(300).await;
            search_query.set(value);
        });
        debounce_task.set(Some(task));
    };

    let on_add = move |username: String| {
        if own().contains(&username) || other().contains(&username) {
            return;
        }
        own.write().push(username);
        search_input.set(String::new());
        search_query.set(String::new());
    };

    rsx! {
        section { class: "moderation-panel moderation-team-editor {accent}",
            h2 { "{title}" }
            div { class: "moderation-chips",
                if own().is_empty() {
                    span { class: "moderation-meta", "Équipe vide" }
                }
                for (index, username) in own().iter().cloned().enumerate() {
                    span { class: "moderation-chip {accent}", key: "{username}",
                        "{username}"
                        button {
                            class: "chip-remove",
                            r#type: "button",
                            aria_label: "Retirer {username}",
                            onclick: move |_| { _ = own.write().remove(index); },
                            img { class: "icon", src: CLOSE_SVG, alt: "" }
                        }
                    }
                }
            }

            input {
                class: "search-input",
                r#type: "search",
                placeholder: "Ajouter un joueur...",
                value: "{search_input}",
                oninput: on_input,
            }

            if !search_input().trim().is_empty() {
                div { class: "moderation-search-results",
                    SuspenseBoundary {
                        fallback: |_| rsx! { p { class: "moderation-meta", "Recherche..." } },
                        PlayerSearchResults { search_query, on_add }
                    }
                }
            }
        }
    }
}

#[component]
fn PlayerSearchResults(search_query: ReadSignal<String>, on_add: EventHandler<String>) -> Element {
    let results = use_server_future(move || {
        let query = search_query();
        async move {
            if query.trim().is_empty() {
                Ok(Vec::<global::UserSearchItem>::new())
            } else {
                global::search_user(query).await
            }
        }
    })?;

    if search_query().trim().is_empty() {
        return rsx! { p { class: "moderation-meta", "Recherche..." } };
    }

    match results() {
        Some(Ok(items)) => rsx! {
            if items.is_empty() {
                p { class: "moderation-meta", "Aucun joueur trouvé" }
            }
            for item in items.iter() {
                button {
                    class: "moderation-search-item",
                    key: "{item.username}",
                    r#type: "button",
                    onclick: {
                        let username = item.username.clone();
                        move |_| on_add.call(username.clone())
                    },
                    span { "{item.username}" }
                    span { class: "chip-elo", "{item.elo}" }
                }
            }
        },
        Some(Err(error)) => rsx! { p { class: "form-error", "{error}" } },
        None => rsx! { p { class: "moderation-meta", "Recherche..." } },
    }
}
