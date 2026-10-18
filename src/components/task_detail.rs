use crate::backend::app_endpoints::{
    get_my_project_member, get_project_assignable_users, get_task, get_task_messages,
    post_task_message, update_task,
};
use crate::components::assignee_search::{AssigneeBadgeList, AssigneeSearchDropdown};
use crate::components::status_select::TaskStatusSelect;
use crate::types::{has_perm, Permission, Task, TaskStatus, User};
use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn TaskPage(id: i64) -> Element {
    let mut task_resource = use_server_future(move || async move { get_task(id).await })?;

    let mut messages_resource =
        use_server_future(move || async move { get_task_messages(id).await })?;

    let task_read = task_resource.read();
    let current_task = match &*task_read {
        Some(Ok(t)) => Some(t.clone()),
        _ => None,
    };

    let pid = current_task.as_ref().map(|t| t.project_id);

    let users_resource =
        use_server_future(move || async move { get_project_assignable_users(pid).await })?;

    let my_membership_resource = use_server_future(move || async move {
        match pid {
            Some(p) => get_my_project_member(p).await.unwrap_or(None),
            None => None,
        }
    })?;

    let my_membership = match &*my_membership_resource.read() {
        Some(Some(m)) => Some(m.clone()),
        _ => None,
    };

    let can_edit = if pid.is_some() {
        my_membership
            .as_ref()
            .map(|m| has_perm(m.permissions_mask, Permission::EXECUTE))
            .unwrap_or(false)
    } else {
        true
    };

    let can_comment = if pid.is_some() {
        my_membership
            .as_ref()
            .map(|m| has_perm(m.permissions_mask, Permission::SUGGEST))
            .unwrap_or(false)
    } else {
        true
    };

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
            TaskDetailNav { task: current_task.clone() }

            if let Some(task) = current_task {
                div { class: "task-detail-card",
                    TaskDetailHeader {
                        task: task.clone(),
                        is_editing: *is_editing.read(),
                        can_edit,
                        is_saving: *is_saving.read(),
                        edit_title: edit_title.read().clone(),
                        on_title_change: move |t| edit_title.set(t),
                        on_start_edit: {
                            let t = task.clone();
                            move |_| start_editing(&t)
                        },
                        on_save: save_task_changes,
                        on_cancel: cancel_editing,
                    }

                    if let Some(err) = &*save_error.read() {
                        div {
                            class: "task-detail-error",
                            style: "margin-bottom: 12px;",
                            strong { "Error saving changes: " }
                            span { "{err}" }
                        }
                    }

                    if *is_editing.read() {
                        TaskEditForm {
                            edit_status: *edit_status.read(),
                            edit_desc: edit_desc.read().clone(),
                            edit_assignees: edit_assignees.read().clone(),
                            task_title: edit_title.read().clone(),
                            all_users: all_users.clone(),
                            on_status_change: move |new_st| edit_status.set(new_st),
                            on_desc_change: move |new_desc| edit_desc.set(new_desc),
                            on_assign: move |uid| {
                                if let Some(u) = all_users.iter().find(|user| user.id == uid) {
                                    edit_assignees.write().push(u.clone());
                                }
                            },
                            on_unassign: move |uid| {
                                edit_assignees.write().retain(|u| u.id != uid);
                            },
                        }
                    } else {
                        TaskViewBody { task }
                    }
                }

                TaskDiscussionSection {
                    messages,
                    can_comment,
                    is_posting: *is_posting_message.read(),
                    message_text: new_message_text.read().clone(),
                    error: (*message_error.read()).clone(),
                    on_text_change: move |val| new_message_text.set(val),
                    on_post: handle_post_message,
                }
            } else if let Some(Err(err)) = &*task_resource.read() {
                TaskAccessDenied { err: err.to_string() }
            } else {
                div { class: "task-detail-loading",
                    p { "Loading task details..." }
                }
            }
        }
    }
}

/// Breadcrumb navigation bar above task details.
#[component]
fn TaskDetailNav(task: Option<Task>) -> Element {
    rsx! {
        div {
            class: "task-detail-nav",
            style: "display: flex; gap: 12px; align-items: center;",
            Link { to: Route::TasksDashboardPage {}, class: "btn-back-link", "← My Tasks" }
            if let Some(t) = task {
                span { style: "color: var(--text-muted); font-size: var(--font-size-meta);", "•" }
                Link {
                    to: Route::ProjectTasksPage {
                        project_id: t.project_id,
                    },
                    class: "btn-back-link",
                    "📁 {t.project_name}"
                }
            }
        }
    }
}

/// Header section with title and actions (view or edit mode).
#[component]
fn TaskDetailHeader(
    task: Task,
    is_editing: bool,
    can_edit: bool,
    is_saving: bool,
    edit_title: String,
    on_title_change: EventHandler<String>,
    on_start_edit: EventHandler<()>,
    on_save: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "task-detail-header",
            if is_editing {
                div {
                    class: "task-detail-title-group",
                    style: "width: 100%;",
                    label { class: "form-label", "Title" }
                    input {
                        class: "task-edit-title-input",
                        value: "{edit_title}",
                        oninput: move |e| on_title_change.call(e.value()),
                    }
                }
            } else {
                div { class: "task-detail-title-group",
                    h1 { class: "task-detail-title", "{task.title}" }
                    span { class: "task-detail-meta",
                        "Task #{task.id} • Project: "
                        Link {
                            to: Route::ProjectTasksPage {
                                project_id: task.project_id,
                            },
                            style: "color: var(--primary-light); text-decoration: none; font-weight: var(--font-weight-bold);",
                            "📁 {task.project_name}"
                        }
                        " • Created by "
                        strong { "{task.author_name}" }
                    }
                }
            }

            div { class: "task-detail-actions",
                if is_editing {
                    button {
                        r#type: "button",
                        class: "btn-task-save",
                        disabled: is_saving,
                        onclick: move |_| on_save.call(()),
                        if is_saving { "Saving..." } else { "Save" }
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        disabled: is_saving,
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                } else {
                    span { class: "task-detail-status status-{task.status.as_str()}", "{task.status}" }
                    if can_edit {
                        button {
                            r#type: "button",
                            class: "btn-task-edit",
                            onclick: move |_| on_start_edit.call(()),
                            "✎ Edit Task"
                        }
                    }
                }
            }
        }
    }
}

/// Editable sections of task details: status, description, and assignees.
#[component]
fn TaskEditForm(
    edit_status: TaskStatus,
    edit_desc: String,
    edit_assignees: Vec<User>,
    task_title: String,
    all_users: Vec<User>,
    on_status_change: EventHandler<TaskStatus>,
    on_desc_change: EventHandler<String>,
    on_assign: EventHandler<i64>,
    on_unassign: EventHandler<i64>,
) -> Element {
    rsx! {
        div { class: "task-detail-section",
            label { class: "section-title", "Status" }
            TaskStatusSelect {
                status: edit_status,
                can_mutate: true,
                on_change: move |new_st| on_status_change.call(new_st),
            }
        }

        div { class: "task-detail-section",
            label { class: "section-title", "Description" }
            textarea {
                class: "task-edit-description-textarea",
                value: "{edit_desc}",
                placeholder: "Add a clear description of the task...",
                oninput: move |e| on_desc_change.call(e.value()),
            }
        }

        div { class: "task-detail-section",
            label { class: "section-title", "Assignees" }
            AssigneeBadgeList {
                assignees: edit_assignees.clone(),
                task_title: task_title.clone(),
                can_mutate: true,
                on_unassign: move |uid| on_unassign.call(uid),
            }
            AssigneeSearchDropdown {
                all_users,
                current_assignees: edit_assignees,
                task_title,
                on_assign: move |uid| on_assign.call(uid),
            }
        }
    }
}

/// Read-only body sections of task details (description and assignees).
#[component]
fn TaskViewBody(task: Task) -> Element {
    rsx! {
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
                        li {
                            key: "{user.id}",
                            class: "detail-assignee-item",
                            span { class: "detail-assignee-name", "{user.name}" }
                            span { class: "detail-assignee-email", "({user.email})" }
                        }
                    }
                }
            }
        }
    }
}

/// Interactive task discussion thread and message composer.
#[component]
fn TaskDiscussionSection(
    messages: Vec<crate::types::TaskMessage>,
    can_comment: bool,
    is_posting: bool,
    message_text: String,
    error: Option<String>,
    on_text_change: EventHandler<String>,
    on_post: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "task-discussion-container",
            h2 { class: "task-discussion-title", "Discussion Thread ({messages.len()})" }

            div { class: "task-discussion-messages",
                if messages.is_empty() {
                    p { class: "empty-description",
                        "No messages in this discussion yet. Start the conversation below!"
                    }
                } else {
                    for msg in messages {
                        div {
                            key: "{msg.msg_id}",
                            class: "task-discussion-message-item",
                            div { class: "task-message-header",
                                span { class: "task-message-author",
                                    "#{msg.msg_id} • {msg.author_name}"
                                }
                                span { class: "task-message-time", "just now" }
                            }
                            p { class: "task-message-body", "{msg.msg}" }
                        }
                    }
                }
            }

            if can_comment {
                div { class: "task-message-composer",
                    if let Some(err) = error {
                        div {
                            class: "task-detail-error",
                            style: "padding: 8px 12px;",
                            "{err}"
                        }
                    }
                    textarea {
                        class: "task-message-input",
                        placeholder: "Write a message or update on this task...",
                        value: "{message_text}",
                        oninput: move |e| on_text_change.call(e.value()),
                        disabled: is_posting,
                    }
                    button {
                        r#type: "button",
                        class: "btn-post-message",
                        disabled: is_posting,
                        onclick: move |_| on_post.call(()),
                        if is_posting { "Posting..." } else { "Post Message" }
                    }
                }
            } else {
                div {
                    style: "padding: 12px 16px; background-color: var(--bg-surface-raised); border-radius: var(--radius-sm); border: 1px solid var(--border-subtle); color: var(--text-muted); font-size: var(--font-size-body-sm); margin-top: 12px;",
                    "You have read-only access and cannot post messages in this project discussion."
                }
            }
        }
    }
}

/// Card displayed when the user does not have permission to view the task.
#[component]
fn TaskAccessDenied(err: String) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            style: "border: 1px solid var(--danger-bg); margin-top: 16px; background-color: var(--bg-surface); padding: 24px; border-radius: var(--radius-md);",
            h2 { style: "color: var(--danger); margin-bottom: 8px; font-size: var(--font-size-h3);", "Access Denied" }
            p { style: "color: var(--text-muted); font-size: var(--font-size-body);", "{err}" }
            div { style: "margin-top: 16px;",
                Link { to: Route::TasksDashboardPage {}, class: "btn-back-link", "← Return to My Tasks" }
            }
        }
    }
}
