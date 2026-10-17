use dioxus::prelude::*;
use crate::types::{OrganizationMember, User};
use crate::backend::app_endpoints::{
    assign_organization_member_role, create_organization_role,
    invite_user_to_organization, request_organization_governance_transfer,
};

#[component]
pub fn InviteOrgMemberModal(
    organization_id: i64,
    all_users: Vec<User>,
    available_roles: Vec<String>,
    on_close: EventHandler<()>,
    on_invited: EventHandler<()>,
) -> Element {
    let mut selected_user_id = use_signal(|| Option::<i64>::None);
    let mut selected_role = use_signal(|| "member".to_string());
    let mut message = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_submitting = use_signal(|| false);

    let handle_submit = move |_| {
        let uid = match *selected_user_id.read() {
            Some(id) => id,
            None => {
                error_msg.set(Some("Please select a user to invite.".to_string()));
                return;
            }
        };

        let role_val = selected_role.read().clone();
        let msg_val = message.read().trim().to_string();

        is_submitting.set(true);
        error_msg.set(None);

        spawn(async move {
            match invite_user_to_organization(organization_id, uid, role_val, msg_val).await {
                Ok(_) => {
                    is_submitting.set(false);
                    on_invited.call(());
                }
                Err(e) => {
                    is_submitting.set(false);
                    error_msg.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { class: "overlay-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 520px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", "Invite Member to Organization" }
                        p { class: "overlay-subtitle", "Send an official invitation with an initial role." }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        "✕"
                    }
                }
                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 16px;",
                    if let Some(err) = &*error_msg.read() {
                        div { class: "task-detail-error", "{err}" }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Select User" }
                        select {
                            class: "task-status-select",
                            onchange: move |e| {
                                if let Ok(id) = e.value().parse::<i64>() {
                                    selected_user_id.set(Some(id));
                                } else {
                                    selected_user_id.set(None);
                                }
                            },
                            option { value: "", "-- Choose user to invite --" }
                            for u in &all_users {
                                option { value: "{u.id}", "{u.name} ({u.email})" }
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Initial Base Role" }
                        select {
                            class: "task-status-select",
                            value: "{selected_role}",
                            onchange: move |e| selected_role.set(e.value()),
                            option { value: "member", "Member (Default)" }
                            option { value: "admin", "Admin" }
                            for r in &available_roles {
                                if r != "admin" && r != "member" {
                                    option { value: "{r}", "{r}" }
                                }
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Invitation Note (Optional)" }
                        textarea {
                            class: "task-message-input",
                            placeholder: "Welcome note or department information...",
                            value: "{message}",
                            oninput: move |e| message.set(e.value()),
                        }
                    }
                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            onclick: move |_| on_close.call(()),
                            "Cancel"
                        }
                        button {
                            r#type: "button",
                            class: "btn-task-save",
                            disabled: *is_submitting.read(),
                            onclick: handle_submit,
                            if *is_submitting.read() { "Sending..." } else { "Send Invitation" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn CreateOrgRoleModal(
    organization_id: i64,
    on_close: EventHandler<()>,
    on_created: EventHandler<()>,
) -> Element {
    let mut role_name = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_submitting = use_signal(|| false);

    let handle_submit = move |_| {
        let name_clean = role_name.read().trim().to_lowercase();
        if name_clean.is_empty() {
            error_msg.set(Some("Role name cannot be empty.".to_string()));
            return;
        }

        is_submitting.set(true);
        error_msg.set(None);

        spawn(async move {
            match create_organization_role(organization_id, name_clean).await {
                Ok(_) => {
                    is_submitting.set(false);
                    on_created.call(());
                }
                Err(e) => {
                    is_submitting.set(false);
                    error_msg.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { class: "overlay-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 480px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", "Define Custom Role" }
                        p { class: "overlay-subtitle", "Create a specialized functional role (e.g. engineering, marketing, auditor)." }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        "✕"
                    }
                }
                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 16px;",
                    if let Some(err) = &*error_msg.read() {
                        div { class: "task-detail-error", "{err}" }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Role Identifier" }
                        input {
                            class: "task-edit-title-input",
                            placeholder: "e.g. engineering, marketing, qa-lead",
                            value: "{role_name}",
                            oninput: move |e| role_name.set(e.value()),
                        }
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted); margin-top: 4px;",
                            "Roles can be assigned to members and mapped to project permission rules."
                        }
                    }
                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            onclick: move |_| on_close.call(()),
                            "Cancel"
                        }
                        button {
                            r#type: "button",
                            class: "btn-task-save",
                            disabled: *is_submitting.read(),
                            onclick: handle_submit,
                            if *is_submitting.read() { "Creating..." } else { "Create Role" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn AssignMemberRoleModal(
    organization_id: i64,
    member: OrganizationMember,
    available_roles: Vec<String>,
    on_close: EventHandler<()>,
    on_assigned: EventHandler<()>,
) -> Element {
    let mut selected_role = use_signal(|| {
        available_roles.first().cloned().unwrap_or_else(|| "member".to_string())
    });
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_submitting = use_signal(|| false);

    let member_user_id = member.user_id;

    let handle_submit = move |_| {
        let r = selected_role.read().clone();
        if r.is_empty() {
            error_msg.set(Some("Please select a role.".to_string()));
            return;
        }

        is_submitting.set(true);
        error_msg.set(None);

        spawn(async move {
            match assign_organization_member_role(organization_id, member_user_id, r).await {
                Ok(_) => {
                    is_submitting.set(false);
                    on_assigned.call(());
                }
                Err(e) => {
                    is_submitting.set(false);
                    error_msg.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { class: "overlay-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 480px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", "Assign Role to {member.user_name}" }
                        p { class: "overlay-subtitle", "Add an additional functional role to this user." }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        "✕"
                    }
                }
                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 16px;",
                    if let Some(err) = &*error_msg.read() {
                        div { class: "task-detail-error", "{err}" }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Role to Assign" }
                        select {
                            class: "task-status-select",
                            value: "{selected_role}",
                            onchange: move |e| selected_role.set(e.value()),
                            for r in &available_roles {
                                option { value: "{r}", "{r}" }
                            }
                        }
                    }
                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            onclick: move |_| on_close.call(()),
                            "Cancel"
                        }
                        button {
                            r#type: "button",
                            class: "btn-task-save",
                            disabled: *is_submitting.read(),
                            onclick: handle_submit,
                            if *is_submitting.read() { "Assigning..." } else { "Assign Role" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn TransferOrgGovernanceModal(
    organization_id: i64,
    members: Vec<OrganizationMember>,
    current_governor_id: i64,
    on_close: EventHandler<()>,
    on_transferred: EventHandler<()>,
) -> Element {
    let mut selected_successor_id = use_signal(|| Option::<i64>::None);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut is_submitting = use_signal(|| false);

    let candidates: Vec<OrganizationMember> = members
        .into_iter()
        .filter(|m| m.user_id != current_governor_id)
        .collect();

    let handle_submit = move |_| {
        let successor_id = match *selected_successor_id.read() {
            Some(id) => id,
            None => {
                error_msg.set(Some("Please select a successor member.".to_string()));
                return;
            }
        };

        is_submitting.set(true);
        error_msg.set(None);

        spawn(async move {
            match request_organization_governance_transfer(organization_id, successor_id).await {
                Ok(_) => {
                    is_submitting.set(false);
                    on_transferred.call(());
                }
                Err(e) => {
                    is_submitting.set(false);
                    error_msg.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { class: "overlay-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 500px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", "Transfer Organization Governance" }
                        p { class: "overlay-subtitle", "Nominate a successor governor. They must accept before governance transfers." }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        "✕"
                    }
                }
                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 16px;",
                    if let Some(err) = &*error_msg.read() {
                        div { class: "task-detail-error", "{err}" }
                    }
                    if candidates.is_empty() {
                        p { style: "color: var(--text-muted); font-size: var(--font-size-body-sm);",
                            "There are no other members in this organization yet. Invite members first before initiating governance transfer."
                        }
                    } else {
                        div { class: "form-group",
                            label { class: "form-label", "Select Successor Member" }
                            select {
                                class: "task-status-select",
                                onchange: move |e| {
                                    if let Ok(id) = e.value().parse::<i64>() {
                                        selected_successor_id.set(Some(id));
                                    } else {
                                        selected_successor_id.set(None);
                                    }
                                },
                                option { value: "", "-- Choose successor --" }
                                for c in &candidates {
                                    option { value: "{c.user_id}", "{c.user_name} ({c.user_email})" }
                                }
                            }
                        }
                    }
                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            onclick: move |_| on_close.call(()),
                            "Cancel"
                        }
                        if !candidates.is_empty() {
                            button {
                                r#type: "button",
                                class: "btn-task-save",
                                disabled: *is_submitting.read(),
                                onclick: handle_submit,
                                if *is_submitting.read() { "Sending Request..." } else { "Nominate Successor" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ConfirmActiveTasksWarningModal(
    warning_message: String,
    on_close: EventHandler<()>,
    on_confirm: EventHandler<()>,
    is_loading: bool,
) -> Element {
    rsx! {
        div { class: "overlay-backdrop", style: "z-index: 1200;",
            onclick: move |_| on_close.call(()),
            div {
                class: "overlay-modal",
                style: "max-width: 520px; border-top: 4px solid var(--status-in_progress);",
                onclick: move |e| e.stop_propagation(),
                div { class: "overlay-header",
                    div { class: "overlay-title-group",
                        h2 { class: "overlay-title", "⚠️ Active Task Impact Warning" }
                        p { class: "overlay-subtitle", "Review active tasks that will be impacted by this change." }
                    }
                    button {
                        r#type: "button",
                        class: "btn-close-overlay",
                        onclick: move |_| on_close.call(()),
                        "✕"
                    }
                }
                div { style: "padding: 20px 24px; display: flex; flex-direction: column; gap: 14px;",
                    div {
                        style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-default); border-radius: var(--radius-sm); padding: 12px; font-size: var(--font-size-body-sm); color: var(--text-primary);",
                        "{warning_message}"
                    }
                    p { style: "font-size: var(--font-size-meta); color: var(--text-muted); margin: 0;",
                        "Proceeding will revoke project role grants and automatically unassign non-in-progress tasks for affected users."
                    }
                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-task-cancel",
                            onclick: move |_| on_close.call(()),
                            disabled: is_loading,
                            "Cancel & Keep As Is"
                        }
                        button {
                            r#type: "button",
                            class: "btn-task-save",
                            style: "background-color: var(--danger); border-color: var(--danger);",
                            disabled: is_loading,
                            onclick: move |_| on_confirm.call(()),
                            if is_loading { "Processing..." } else { "Confirm & Proceed Anyway" }
                        }
                    }
                }
            }
        }
    }
}
