use crate::backend::app_endpoints::{get_project_invite_directory, invite_user_to_project};
use dioxus::prelude::*;

#[component]
pub fn InviteProjectMemberModal(
    project_id: i64,
    #[props(default)] project_name: Option<String>,
    on_close: EventHandler<()>,
    #[props(default)] on_invited: EventHandler<()>,
) -> Element {
    let dir_resource =
        use_server_future(move || async move { get_project_invite_directory(project_id).await })?;

    // Filter state: None = All, Some(-1) = External, Some(id) = Specific Org
    let mut selected_org_filter = use_signal(|| Option::<i64>::None);
    let mut invite_user_id = use_signal(|| Option::<i64>::None);
    let mut invite_role = use_signal(|| "member".to_string());
    let mut invite_message = use_signal(String::new);
    let mut invite_feedback = use_signal(|| Option::<Result<String, String>>::None);
    let mut is_inviting = use_signal(|| false);

    let directory = match &*dir_resource.read() {
        Some(Ok(dir)) => Some(dir.clone()),
        _ => None,
    };

    let organizations = directory
        .as_ref()
        .map(|d| d.organizations.clone())
        .unwrap_or_default();
    let candidates = directory
        .as_ref()
        .map(|d| d.candidates.clone())
        .unwrap_or_default();

    // Filter candidates based on organization selection
    let filtered_candidates = {
        let filter = *selected_org_filter.read();
        candidates
            .iter()
            .filter(|c| match filter {
                None => true,
                Some(-1) => c.organization_ids.is_empty(),
                Some(target_oid) => c.organization_ids.contains(&target_oid),
            })
            .cloned()
            .collect::<Vec<_>>()
    };

    let handle_send_invite = move |_| {
        let target_uid = match *invite_user_id.read() {
            Some(id) => id,
            None => {
                invite_feedback.set(Some(Err("Please select a user to invite.".to_string())));
                return;
            }
        };

        let role_val = invite_role.read().clone();
        let msg_val = invite_message.read().trim().to_string();

        is_inviting.set(true);
        invite_feedback.set(None);

        spawn(async move {
            match invite_user_to_project(project_id, target_uid, role_val, msg_val).await {
                Ok(_) => {
                    invite_feedback.set(Some(Ok("Invitation sent successfully!".to_string())));
                    invite_message.write().clear();
                    is_inviting.set(false);
                    on_invited.call(());
                }
                Err(err) => {
                    invite_feedback.set(Some(Err(err.to_string())));
                    is_inviting.set(false);
                }
            }
        });
    };

    let title_text = match &project_name {
        Some(name) => format!("Invite Member to \"{name}\""),
        None => "Invite Member to Project".to_string(),
    };

    rsx! {
        div { class: "overlay-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 560px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", "{title_text}" }
                        p { class: "overlay-subtitle", "Collaborate with external users or team members from affiliated organizations." }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        "✕"
                    }
                }
                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 16px;",
                    ProjectInviteFeedback { feedback: (*invite_feedback.read()).clone() }

                    ProjectInviteForm {
                        organizations,
                        filtered_candidates,
                        selected_org_filter: *selected_org_filter.read(),
                        selected_user_id: *invite_user_id.read(),
                        invite_role: invite_role.read().clone(),
                        invite_message: invite_message.read().clone(),
                        is_inviting: *is_inviting.read(),
                        on_filter_change: move |f| {
                            selected_org_filter.set(f);
                            invite_user_id.set(None);
                        },
                        on_user_change: move |uid| invite_user_id.set(uid),
                        on_role_change: move |r| invite_role.set(r),
                        on_message_change: move |m| invite_message.set(m),
                        on_close: move |_| on_close.call(()),
                        on_submit: handle_send_invite,
                    }
                }
            }
        }
    }
}

/// Alert message displaying success or error result of sending an invitation.
#[component]
fn ProjectInviteFeedback(feedback: Option<Result<String, String>>) -> Element {
    if let Some(res) = feedback {
        match res {
            Ok(msg) => rsx! {
                div { class: "task-detail-status status-finished", style: "padding: 8px 12px; margin-bottom: 8px;",
                    "✓ {msg}"
                }
            },
            Err(err) => rsx! {
                div { class: "task-detail-error", style: "padding: 8px 12px; margin-bottom: 8px;",
                    "{err}"
                }
            },
        }
    } else {
        rsx! {}
    }
}

/// Form controls for inviting a user to a project with organization filtering and role assignment.
#[component]
fn ProjectInviteForm(
    organizations: Vec<crate::types::OrganizationSummary>,
    filtered_candidates: Vec<crate::types::ProjectInviteCandidate>,
    selected_org_filter: Option<i64>,
    selected_user_id: Option<i64>,
    invite_role: String,
    invite_message: String,
    is_inviting: bool,
    on_filter_change: EventHandler<Option<i64>>,
    on_user_change: EventHandler<Option<i64>>,
    on_role_change: EventHandler<String>,
    on_message_change: EventHandler<String>,
    on_close: EventHandler<()>,
    on_submit: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "form-group",
            label { class: "form-label", "Filter by Organization" }
            select {
                class: "task-status-select",
                value: match selected_org_filter {
                    None => "".to_string(),
                    Some(id) => id.to_string(),
                },
                onchange: move |e| {
                    let val = e.value();
                    if val.is_empty() {
                        on_filter_change.call(None);
                    } else if let Ok(parsed) = val.parse::<i64>() {
                        on_filter_change.call(Some(parsed));
                    }
                },
                option { value: "", "🌐 All Users (No Filter)" }
                option { value: "-1", "👤 External Users Only (No Organization)" }
                for org in &organizations {
                    option { value: "{org.id}", "🏢 {org.name} (@{org.slug})" }
                }
            }
        }

        div { class: "form-group",
            label { class: "form-label",
                "Select User "
                span { style: "font-weight: normal; color: var(--text-muted); font-size: var(--font-size-meta);",
                    "({filtered_candidates.len()} available)"
                }
            }
            select {
                class: "task-status-select",
                onchange: move |e| {
                    let val = e.value();
                    if let Ok(uid) = val.parse::<i64>() {
                        on_user_change.call(Some(uid));
                    } else {
                        on_user_change.call(None);
                    }
                },
                option { value: "", "-- Choose user to invite --" }
                for user in &filtered_candidates {
                    option {
                        value: "{user.id}",
                        disabled: user.is_member,
                        if user.is_member {
                            "{user.name} ({user.email}) — {user.organizations_display} [Already a member]"
                        } else {
                            "{user.name} ({user.email}) — {user.organizations_display}"
                        }
                    }
                }
            }
        }

        div { class: "form-group",
            label { class: "form-label", "Role" }
            select {
                class: "task-status-select",
                value: "{invite_role}",
                onchange: move |e| on_role_change.call(e.value()),
                option { value: "member", "Member (Create & Edit Tasks)" }
                option { value: "viewer", "Viewer (Read Only)" }
                option { value: "admin", "Admin (Manage Settings & Members)" }
            }
        }

        div { class: "form-group",
            label { class: "form-label", "Personal Message (Optional)" }
            textarea {
                class: "task-message-input",
                placeholder: "e.g. Join our project to collaborate on these tasks!",
                value: "{invite_message}",
                oninput: move |e| on_message_change.call(e.value()),
            }
        }

        div { class: "overlay-footer",
            button {
                r#type: "button",
                class: "btn-cancel",
                onclick: move |_| on_close.call(()),
                "Close"
            }
            button {
                r#type: "button",
                class: "btn-task-save",
                disabled: is_inviting || selected_user_id.is_none(),
                onclick: move |_| on_submit.call(()),
                if is_inviting { "Sending..." } else { "Send Invitation" }
            }
        }
    }
}
