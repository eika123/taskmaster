use dioxus::prelude::*;
use crate::backend::app_endpoints::{invite_user_to_project, suggest_task_for_project};
use crate::types::User;

#[component]
pub fn InlineCreateTaskModal(
    project_id: Option<i64>,
    on_close: EventHandler<()>,
    on_created: EventHandler<()>,
) -> Element {
    let mut new_task_title = use_signal(String::new);
    let mut new_task_desc = use_signal(String::new);
    let mut new_task_err = use_signal(|| Option::<String>::None);
    let mut is_creating_task = use_signal(|| false);

    let handle_create_task = move |_| {
        let title = new_task_title.read().trim().to_string();
        let desc = new_task_desc.read().trim().to_string();
        if title.is_empty() {
            new_task_err.set(Some("Task title is required.".to_string()));
            return;
        }

        is_creating_task.set(true);
        new_task_err.set(None);

        let target_pid = project_id.unwrap_or(1);
        spawn(async move {
            match suggest_task_for_project(target_pid, title, desc).await {
                Ok(_) => {
                    new_task_title.write().clear();
                    new_task_desc.write().clear();
                    is_creating_task.set(false);
                    on_created.call(());
                }
                Err(err) => {
                    new_task_err.set(Some(err.to_string()));
                    is_creating_task.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--primary); margin-bottom: 20px;",
            h2 { class: "dashboard-panel-title", "Create a Task" }
            if let Some(err) = &*new_task_err.read() {
                div { class: "task-detail-error", "{err}" }
            }
            div { style: "display: flex; flex-direction: column; gap: 12px;",
                input {
                    class: "task-edit-title-input",
                    placeholder: "Task Title (e.g. Implement OAuth Flow)",
                    value: "{new_task_title}",
                    oninput: move |e| new_task_title.set(e.value()),
                }
                textarea {
                    class: "task-edit-description-textarea",
                    placeholder: "Describe requirements, criteria, or context...",
                    value: "{new_task_desc}",
                    oninput: move |e| new_task_desc.set(e.value()),
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
                        disabled: *is_creating_task.read(),
                        onclick: handle_create_task,
                        if *is_creating_task.read() { "Creating..." } else { "Create Task" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn InlineInviteModal(
    project_id: i64,
    current_users: Vec<User>,
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
            h2 { class: "dashboard-panel-title", "Invite User to Project" }
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
                    label { style: "font-size: 0.85rem; color: var(--text-muted); display: block; margin-bottom: 4px;", "Select User" }
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
                        for u in &current_users {
                            option { value: "{u.id}", "{u.name} ({u.email})" }
                        }
                    }
                }
                div {
                    label { style: "font-size: 0.85rem; color: var(--text-muted); display: block; margin-bottom: 4px;", "Role" }
                    select {
                        class: "task-status-select",
                        value: "{invite_role}",
                        onchange: move |e| invite_role.set(e.value()),
                        option { value: "member", "Member" }
                        option { value: "admin", "Admin" }
                    }
                }
                div {
                    label { style: "font-size: 0.85rem; color: var(--text-muted); display: block; margin-bottom: 4px;", "Personal Message (Optional)" }
                    textarea {
                        class: "task-message-input",
                        placeholder: "e.g. Hey, join our project to collaborate on these tasks!",
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

