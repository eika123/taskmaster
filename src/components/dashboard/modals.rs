use dioxus::prelude::*;
use crate::backend::app_endpoints::{create_organization, create_project, invite_user_to_project};
use crate::types::{Organization, User};

#[component]
pub fn CreateProjectModal(
    organizations: Vec<Organization>,
    on_close: EventHandler<()>,
    on_created: EventHandler<()>,
) -> Element {
    let mut new_proj_name = use_signal(String::new);
    let mut new_proj_desc = use_signal(String::new);
    let mut new_proj_org_id = use_signal(|| Option::<i64>::None);
    let mut proj_form_error = use_signal(|| Option::<String>::None);
    let mut is_creating_proj = use_signal(|| false);

    let handle_create = move |_| {
        let name = new_proj_name.read().trim().to_string();
        let desc = new_proj_desc.read().trim().to_string();
        let org_id = *new_proj_org_id.read();

        if name.is_empty() {
            proj_form_error.set(Some("Project name is required.".to_string()));
            return;
        }

        is_creating_proj.set(true);
        proj_form_error.set(None);

        spawn(async move {
            match create_project(name, desc, org_id).await {
                Ok(_) => {
                    new_proj_name.write().clear();
                    new_proj_desc.write().clear();
                    new_proj_org_id.set(None);
                    is_creating_proj.set(false);
                    on_created.call(());
                }
                Err(err) => {
                    proj_form_error.set(Some(err.to_string()));
                    is_creating_proj.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--primary);",
            h2 { class: "dashboard-panel-title", "Create a New Project" }
            if let Some(err) = &*proj_form_error.read() {
                div { class: "task-detail-error", "{err}" }
            }
            div { style: "display: flex; flex-direction: column; gap: 12px;",
                div {
                    label { class: "form-label", "Project Name" }
                    input {
                        class: "task-edit-title-input",
                        placeholder: "e.g. Core Engine",
                        value: "{new_proj_name}",
                        oninput: move |e| new_proj_name.set(e.value()),
                    }
                }
                div {
                    label { class: "form-label", "Description" }
                    textarea {
                        class: "task-edit-description-textarea",
                        placeholder: "What will this project accomplish?",
                        value: "{new_proj_desc}",
                        oninput: move |e| new_proj_desc.set(e.value()),
                    }
                }
                if !organizations.is_empty() {
                    div {
                        label { class: "form-label", "Owning Organization (Optional)" }
                        select {
                            class: "task-status-select",
                            onchange: move |e| {
                                let val = e.value();
                                if val.is_empty() {
                                    new_proj_org_id.set(None);
                                } else if let Ok(org_id) = val.parse::<i64>() {
                                    new_proj_org_id.set(Some(org_id));
                                }
                            },
                            option { value: "", "Standalone (Personal Project)" }
                            for org in &organizations {
                                option { value: "{org.id}", "{org.name}" }
                            }
                        }
                    }
                }
                div { style: "display: flex; gap: 8px; justify-content: flex-end;",
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-save",
                        disabled: *is_creating_proj.read(),
                        onclick: handle_create,
                        if *is_creating_proj.read() { "Creating..." } else { "Create Project" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn CreateOrgModal(
    on_close: EventHandler<()>,
    on_created: EventHandler<()>,
) -> Element {
    let mut new_org_name = use_signal(String::new);
    let mut new_org_slug = use_signal(String::new);
    let mut org_form_error = use_signal(|| Option::<String>::None);
    let mut is_creating_org = use_signal(|| false);

    let handle_create = move |_| {
        let name = new_org_name.read().trim().to_string();
        let slug = new_org_slug.read().trim().to_lowercase();

        if name.is_empty() || slug.is_empty() {
            org_form_error.set(Some("Name and slug are required.".to_string()));
            return;
        }

        is_creating_org.set(true);
        org_form_error.set(None);

        spawn(async move {
            match create_organization(name, slug).await {
                Ok(_) => {
                    new_org_name.write().clear();
                    new_org_slug.write().clear();
                    is_creating_org.set(false);
                    on_created.call(());
                }
                Err(err) => {
                    org_form_error.set(Some(err.to_string()));
                    is_creating_org.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--primary);",
            h2 { class: "dashboard-panel-title", "Create a New Organization" }
            if let Some(err) = &*org_form_error.read() {
                div { class: "task-detail-error", "{err}" }
            }
            div { style: "display: flex; flex-direction: column; gap: 12px;",
                div {
                    label { class: "form-label", "Organization Name" }
                    input {
                        class: "task-edit-title-input",
                        placeholder: "e.g. Acme Corporation",
                        value: "{new_org_name}",
                        oninput: move |e| new_org_name.set(e.value()),
                    }
                }
                div {
                    label { class: "form-label", "Unique Slug" }
                    input {
                        class: "task-edit-title-input",
                        placeholder: "e.g. acme-corp",
                        value: "{new_org_slug}",
                        oninput: move |e| new_org_slug.set(e.value()),
                    }
                }
                div { style: "display: flex; gap: 8px; justify-content: flex-end;",
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-save",
                        disabled: *is_creating_org.read(),
                        onclick: handle_create,
                        if *is_creating_org.read() { "Creating..." } else { "Create Organization" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn InviteMemberModal(
    project_id: i64,
    project_name: String,
    all_users: Vec<User>,
    on_close: EventHandler<()>,
) -> Element {
    let mut invite_user_id = use_signal(|| Option::<i64>::None);
    let mut invite_role = use_signal(|| "member".to_string());
    let mut invite_message = use_signal(String::new);
    let mut invite_feedback = use_signal(|| Option::<Result<String, String>>::None);
    let mut is_inviting = use_signal(|| false);

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
                }
                Err(err) => {
                    invite_feedback.set(Some(Err(err.to_string())));
                    is_inviting.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--status-in_progress); margin-bottom: 20px;",
            h2 { class: "dashboard-panel-title", "Invite Member to \"{project_name}\"" }
            if let Some(res) = &*invite_feedback.read() {
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
            }
            div { style: "display: flex; flex-direction: column; gap: 12px;",
                div {
                    label { class: "form-label", "Select User" }
                    select {
                        class: "task-status-select",
                        onchange: move |e| {
                            let val = e.value();
                            if let Ok(uid) = val.parse::<i64>() {
                                invite_user_id.set(Some(uid));
                            } else {
                                invite_user_id.set(None);
                            }
                        },
                        option { value: "", "-- Choose user to invite --" }
                        for u in &all_users {
                            option { value: "{u.id}", "{u.name} ({u.email})" }
                        }
                    }
                }
                div {
                    label { class: "form-label", "Role" }
                    select {
                        class: "task-status-select",
                        value: "{invite_role}",
                        onchange: move |e| invite_role.set(e.value()),
                        option { value: "member", "Member" }
                        option { value: "admin", "Admin" }
                    }
                }
                div {
                    label { class: "form-label", "Personal Message (Optional)" }
                    textarea {
                        class: "task-message-input",
                        placeholder: "e.g. Join this project to collaborate on these tasks!",
                        value: "{invite_message}",
                        oninput: move |e| invite_message.set(e.value()),
                    }
                }
                div { style: "display: flex; gap: 8px; justify-content: flex-end;",
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-save",
                        disabled: *is_inviting.read(),
                        onclick: handle_send_invite,
                        if *is_inviting.read() { "Sending..." } else { "Send Invitation" }
                    }
                }
            }
        }
    }
}

