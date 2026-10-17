use dioxus::prelude::*;
use crate::backend::app_endpoints::{get_task, get_task_messages, get_users, post_task_message, update_task};
use crate::components::assignee_search::{AssigneeBadgeList, AssigneeSearchDropdown};
use crate::components::status_select::TaskStatusSelect;
use crate::types::{Task, TaskStatus, User};
use crate::Route;

#[component]
pub fn TaskPage(id: i64) -> Element {
    let mut task_resource = use_server_future(move || async move {
        get_task(id).await
    })?;

    let users_resource = use_server_future(move || async move {
        get_users().await
    })?;

    let mut messages_resource = use_server_future(move || async move {
        get_task_messages(id).await
    })?;

    let mut is_editing = use_signal(|| false);
    let mut edit_title = use_signal(String::new);
    let mut edit_desc = use_signal(String::new);
    let mut edit_status = use_signal(|| TaskStatus::Suggested);
    let mut edit_assignees = use_signal(Vec::<User>::new);
    let mut save_error = use_signal(|| Option::<String>::None);
    let mut is_saving = use_signal(|| false);

    let mut new_message_text = use_signal(String::new);
    let mut is_posting_message = use_signal(|| false);
    let mut message_error = use_signal(|| Option::<String>::None);

    let task_read = task_resource.read();
    let current_task = match &*task_read {
        Some(Ok(t)) => Some(t.clone()),
        _ => None,
    };

    let all_users = match &*users_resource.read() {
        Some(Ok(users)) => users.clone(),
        _ => Vec::new(),
    };

    let messages = match &*messages_resource.read() {
        Some(Ok(msgs)) => msgs.clone(),
        _ => Vec::new(),
    };

    let mut start_editing = move |task: &Task| {
        edit_title.set(task.title.clone());
        edit_desc.set(task.description.clone());
        edit_status.set(task.status);
        edit_assignees.set(task.assignees.clone());
        save_error.set(None);
        is_editing.set(true);
    };

    let cancel_editing = move |_| {
        is_editing.set(false);
        save_error.set(None);
    };

    let save_task_changes = move |_| {
        let title_val = edit_title.read().clone();
        let desc_val = edit_desc.read().clone();
        let status_val = *edit_status.read();
        let assignee_ids: Vec<i64> = edit_assignees.read().iter().map(|u| u.id).collect();

        is_saving.set(true);
        save_error.set(None);

        spawn(async move {
            match update_task(id, title_val, desc_val, status_val, assignee_ids).await {
                Ok(_) => {
                    is_editing.set(false);
                    is_saving.set(false);
                    task_resource.restart();
                }
                Err(err) => {
                    save_error.set(Some(err.to_string()));
                    is_saving.set(false);
                }
            }
        });
    };

    let handle_post_message = move |_| {
        let text = new_message_text.read().trim().to_string();
        if text.is_empty() {
            return;
        }

        is_posting_message.set(true);
        message_error.set(None);

        spawn(async move {
            match post_task_message(id, text).await {
                Ok(_) => {
                    new_message_text.write().clear();
                    is_posting_message.set(false);
                    messages_resource.restart();
                }
                Err(err) => {
                    message_error.set(Some(err.to_string()));
                    is_posting_message.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "task-detail-container",
            div { class: "task-detail-nav",
                Link { to: Route::TasksDashboardPage {}, class: "btn-back-link",
                    "← Back to Task Board"
                }
            }

            if let Some(task) = current_task {
                div { class: "task-detail-card",
                    div { class: "task-detail-header",
                        if *is_editing.read() {
                            div { class: "task-detail-title-group", style: "width: 100%;",
                                label { class: "form-label", "Title" }
                                input {
                                    class: "task-edit-title-input",
                                    value: "{edit_title}",
                                    oninput: move |e| edit_title.set(e.value()),
                                }
                            }
                        } else {
                            div { class: "task-detail-title-group",
                                h1 { class: "task-detail-title", "{task.title}" }
                                span { class: "task-detail-meta",
                                    "Task #{task.id} • Created by "
                                    strong { "{task.author_name}" }
                                }
                            }
                        }

                        div { class: "task-detail-actions",
                            if *is_editing.read() {
                                button {
                                    r#type: "button",
                                    class: "btn-task-save",
                                    disabled: *is_saving.read(),
                                    onclick: save_task_changes,
                                    if *is_saving.read() { "Saving..." } else { "Save" }
                                }
                                button {
                                    r#type: "button",
                                    class: "btn-task-cancel",
                                    disabled: *is_saving.read(),
                                    onclick: cancel_editing,
                                    "Cancel"
                                }
                            } else {
                                span { class: "task-detail-status status-{task.status.as_str()}", "{task.status}" }
                                button {
                                    r#type: "button",
                                    class: "btn-task-edit",
                                    onclick: {
                                        let t = task.clone();
                                        move |_| start_editing(&t)
                                    },
                                    "✎ Edit Task"
                                }
                            }
                        }
                    }

                    if let Some(err) = &*save_error.read() {
                        div { class: "task-detail-error", style: "margin-bottom: 12px;",
                            strong { "Error saving changes: " }
                            span { "{err}" }
                        }
                    }

                    if *is_editing.read() {
                        div { class: "task-detail-section",
                            label { class: "section-title", "Status" }
                            TaskStatusSelect {
                                status: *edit_status.read(),
                                on_change: move |new_st| edit_status.set(new_st),
                            }
                        }

                        div { class: "task-detail-section",
                            label { class: "section-title", "Description" }
                            textarea {
                                class: "task-edit-description-textarea",
                                value: "{edit_desc}",
                                placeholder: "Add a clear description of the task...",
                                oninput: move |e| edit_desc.set(e.value()),
                            }
                        }

                        div { class: "task-detail-section",
                            label { class: "section-title", "Assignees" }
                            AssigneeBadgeList {
                                assignees: edit_assignees.read().clone(),
                                task_title: edit_title.read().clone(),
                                on_unassign: move |uid| {
                                    edit_assignees.write().retain(|u| u.id != uid);
                                },
                            }
                            AssigneeSearchDropdown {
                                all_users: all_users.clone(),
                                current_assignees: edit_assignees.read().clone(),
                                task_title: edit_title.read().clone(),
                                on_assign: move |uid| {
                                    if let Some(u) = all_users.iter().find(|user| user.id == uid) {
                                        edit_assignees.write().push(u.clone());
                                    }
                                },
                            }
                        }
                    } else {
                        div { class: "task-detail-section",
                            h2 { class: "section-title", "Description" }
                            if task.description.is_empty() {
                                p { class: "empty-description", "No description provided." }
                            } else {
                                p { class: "task-detail-description-text", "{task.description}" }
                            }
                        }

                        div { class: "task-detail-section",
                            h2 { class: "section-title", "Assignees" }
                            if task.assignees.is_empty() {
                                p { class: "empty-assignees", "No assignees on this task." }
                            } else {
                                ul { class: "detail-assignees-list",
                                    for user in &task.assignees {
                                        li { key: "{user.id}", class: "detail-assignee-item",
                                            span { class: "detail-assignee-name", "{user.name}" }
                                            span { class: "detail-assignee-email", "({user.email})" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Discussion Thread Section
                div { class: "task-discussion-container",
                    h2 { class: "task-discussion-title", "Discussion Thread ({messages.len()})" }

                    div { class: "task-discussion-messages",
                        if messages.is_empty() {
                            p { class: "empty-description", "No messages in this discussion yet. Start the conversation below!" }
                        } else {
                            for msg in messages {
                                div { key: "{msg.msg_id}", class: "task-discussion-message-item",
                                    div { class: "task-message-header",
                                        span { class: "task-message-author", "#{msg.msg_id} • {msg.author_name}" }
                                        span { class: "task-message-time", "just now" }
                                    }
                                    p { class: "task-message-body", "{msg.msg}" }
                                }
                            }
                        }
                    }

                    div { class: "task-message-composer",
                        if let Some(err) = &*message_error.read() {
                            div { class: "task-detail-error", style: "padding: 8px 12px;",
                                "{err}"
                            }
                        }
                        textarea {
                            class: "task-message-input",
                            placeholder: "Write a message or update on this task...",
                            value: "{new_message_text}",
                            oninput: move |e| new_message_text.set(e.value()),
                            disabled: *is_posting_message.read(),
                        }
                        button {
                            r#type: "button",
                            class: "btn-post-message",
                            disabled: *is_posting_message.read(),
                            onclick: handle_post_message,
                            if *is_posting_message.read() { "Posting..." } else { "Post Message" }
                        }
                    }
                }
            } else {
                div { class: "task-detail-loading",
                    p { "Loading task details..." }
                }
            }
        }
    }
}
