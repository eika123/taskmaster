use dioxus::prelude::*;
use super::assignee_search::TaskAssigneesSection;
use super::column_selector::ColumnSelector;
use super::status_select::TaskStatusSelect;
use crate::Route;
use crate::backend::{
    assign_task, delete_task, get_all_tasks, get_users, set_task_status, unassign_task, Task, TaskStatus,
    User,
};


#[component]
pub fn FinishedTasks() -> Element {
    rsx! {
        div { id: "task-status-finished",
            p { "lazy man: FinishedTasks not implemented" }
        }
    }
}

#[component]
pub fn InProgressTasks() -> Element {
    rsx! {
        div { id: "task-status-in-progress",
            p { "lazy man: InProgressTasks not implemented" }
        }
    }
}

#[component]    
pub fn AssignedTasks() -> Element {
    rsx! {
        div { id: "task-status-assigned",
            p { "lazy man: AssignedTasks not implemented" }
        }
    }
}

#[component]
pub fn AcceptedTasks() -> Element {
    rsx! {
        div { id: "task-status-accepted",
            p { "lazy man: AcceptedTasks not implemented" }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskData {
    pub title: String,
    pub description: String,
}

#[component]
pub fn SuggestTaskForm(on_submit: EventHandler<TaskData>) -> Element {
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);

    rsx! {
        form {
            onsubmit: move |e| {
                e.prevent_default();
                let current_title = title.read().trim().to_string();
                let current_desc = description.read().trim().to_string();
                if current_title.is_empty() {
                    *error_msg.write() = Some("Task title cannot be empty".to_string());
                    return;
                }
                *error_msg.write() = None;
                on_submit
                    .call(TaskData {
                        title: current_title,
                        description: current_desc,
                    });
                title.write().clear();
                description.write().clear();
            },
            div { class: "suggest-task-form",
                input {
                    r#type: "text",
                    placeholder: "Enter task title...",
                    value: "{title}",
                    oninput: move |e| *title.write() = e.value(),
                }
                input {
                    r#type: "text",
                    placeholder: "Enter description (optional)...",
                    value: "{description}",
                    oninput: move |e| *description.write() = e.value(),
                }
                button { r#type: "submit", "Suggest Task" }
            }
            if let Some(err) = error_msg() {
                p { class: "error", "{err}" }
            }
        }
    }
}

#[component]
pub fn TaskCard(
    task: Task,
    all_users: Vec<User>,
    on_dismiss: Option<EventHandler<i64>>,
    on_status_change: Option<EventHandler<(u64, TaskStatus)>>,
    on_assign: Option<EventHandler<(i64, i64)>>,
    on_unassign: Option<EventHandler<(i64, i64)>>,
) -> Element {
    let task_id = task.id;
    let title = task.title.clone();
    let author_name = task.author_name.clone();
    let description = task.description.clone();
    let current_status = task.status;
    let assignees = task.assignees.clone();

    rsx! {
        div { class: "task-card",
            div { class: "task-card-content",
                div { class: "task-card-header",
                    div { class: "task-card-title-group",
                        Link {
                            to: Route::TaskPage { id: task_id },
                            class: "task-card-title-link",
                            h3 { class: "task-card-title", "{title}" }
                        }

                        span { class: "task-card-author", "Created by: {author_name}" }
                    }

                    // Status dropdown helper component
                    TaskStatusSelect {
                        status: current_status,
                        aria_label: "Status for task: {title}",
                        on_change: move |new_status| {
                            if let Some(cb) = on_status_change {
                                cb.call((task_id as u64, new_status));
                            }
                        },
                    }
                }

                if !description.is_empty() {
                    div { class: "task-card-body",
                        p { class: "task-card-description", "{description}" }
                    }
                }

                // Assignee badges, search input & dropdown helper component
                TaskAssigneesSection {
                    assignees,
                    all_users,
                    task_title: title.clone(),
                    on_assign: move |user_id| {
                        if let Some(cb) = on_assign {
                            cb.call((task_id, user_id));
                        }
                    },
                    on_unassign: move |user_id| {
                        if let Some(cb) = on_unassign {
                            cb.call((task_id, user_id));
                        }
                    },
                }
            }

            div { class: "task-card-footer",
                button {
                    r#type: "button",
                    class: "btn-dismiss",
                    aria_label: "Dismiss task: {title}",
                    onclick: move |_| {
                        if let Some(cb) = on_dismiss {
                            cb.call(task_id);
                        }
                    },
                    span { class: "btn-dismiss-icon", "aria-hidden": "true", "✕" }
                    span { class: "sr-only", "Dismiss" }
                }
            }
        }
    }
}


// Backward-compatibility alias
#[component]
pub fn SuggestedTaskCard(
    task: Task,
    all_users: Option<Vec<User>>,
    on_dismiss: Option<EventHandler<i64>>,
    on_status_change: Option<EventHandler<(u64, TaskStatus)>>,
    on_assign: Option<EventHandler<(i64, i64)>>,
    on_unassign: Option<EventHandler<(i64, i64)>>,
) -> Element {
    rsx! {
        TaskCard {
            task,
            all_users: all_users.unwrap_or_default(),
            on_dismiss,
            on_status_change,
            on_assign,
            on_unassign,
        }
    }
}

#[component]
pub fn TaskColumn(
    status: TaskStatus,
    tasks: Vec<Task>,
    all_users: Vec<User>,
    on_dismiss: Option<EventHandler<i64>>,
    on_status_change: Option<EventHandler<(u64, TaskStatus)>>,
    on_assign: Option<EventHandler<(i64, i64)>>,
    on_unassign: Option<EventHandler<(i64, i64)>>,
) -> Element {
    let filtered_tasks: Vec<Task> = tasks.into_iter().filter(|t| t.status == status).collect();
    let status_name = match status {
        TaskStatus::Suggested => "Suggested",
        TaskStatus::Accepted => "Accepted",
        TaskStatus::Assigned => "Assigned",
        TaskStatus::InProgress => "In Progress",
        TaskStatus::Finished => "Finished",
    };

    rsx! {
        div { class: "task-column",
            div { class: "task-column-header",
                h2 { class: "task-column-title", "{status_name}" }
                span { class: "task-column-badge", "{filtered_tasks.len()}" }
            }
            if filtered_tasks.is_empty() {
                p { class: "empty-message", "No {status_name.to_lowercase()} tasks" }
            } else {
                ul { class: "task-list", role: "list",
                    for task in filtered_tasks {
                        li { key: "{task.id}", class: "task-list-item",
                            TaskCard {
                                task: task.clone(),
                                all_users: all_users.clone(),
                                on_dismiss: move |id| {
                                    if let Some(cb) = on_dismiss {
                                        cb.call(id);
                                    }
                                },
                                on_status_change: move |args| {
                                    if let Some(cb) = on_status_change {
                                        cb.call(args);
                                    }
                                },
                                on_assign: move |args| {
                                    if let Some(cb) = on_assign {
                                        cb.call(args);
                                    }
                                },
                                on_unassign: move |args| {
                                    if let Some(cb) = on_unassign {
                                        cb.call(args);
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

// Backward-compatibility alias
#[component]
pub fn SuggestedTasks(
    tasks: Vec<Task>,
    all_users: Option<Vec<User>>,
    on_dismiss: Option<EventHandler<i64>>,
    on_status_change: Option<EventHandler<(u64, TaskStatus)>>,
    on_assign: Option<EventHandler<(i64, i64)>>,
    on_unassign: Option<EventHandler<(i64, i64)>>,
) -> Element {
    rsx! {
        TaskColumn {
            status: TaskStatus::Suggested,
            tasks,
            all_users: all_users.unwrap_or_default(),
            on_dismiss,
            on_status_change,
            on_assign,
            on_unassign,
        }
    }
}

#[component]
pub fn TaskView() -> Element {
    let mut selected_statuses = use_signal(|| vec![
        TaskStatus::Suggested,
        TaskStatus::Accepted,
        TaskStatus::Assigned,
        TaskStatus::InProgress,
        TaskStatus::Finished,
    ]);

    let tasks = use_server_future(|| async move {
        get_all_tasks(None, 50).await
    })?;

    let users_resource = use_server_future(|| async move {
        get_users().await
    })?;

    let mut task_list = use_signal(|| {
        match &*tasks.read() {
            Some(Ok(list)) => list.clone(),
            _ => Vec::new(),
        }
    });

    let mut all_users = use_signal(|| {
        match &*users_resource.read() {
            Some(Ok(list)) => list.clone(),
            _ => Vec::new(),
        }
    });

    use_effect(move || {
        if let Some(Ok(list)) = &*tasks.read() {
            task_list.set(list.clone());
        }
    });

    use_effect(move || {
        if let Some(Ok(list)) = &*users_resource.read() {
            all_users.set(list.clone());
        }
    });

    let on_dismiss_handler = move |task_id: i64| {
        task_list.with_mut(|tasks| {
            tasks.retain(|t| t.id != task_id);
        });
        spawn(async move {
            let _ = delete_task(task_id).await;
            if let Ok(fresh) = get_all_tasks(None, 50).await {
                task_list.set(fresh);
            }
        });
    };

    let on_status_change_handler = move |(task_id, new_status): (u64, TaskStatus)| {
        task_list.with_mut(|tasks| {
            if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id as i64) {
                t.status = new_status;
            }
        });
        spawn(async move {
            match set_task_status(task_id, new_status).await {
                Ok(_) => {
                    if let Ok(fresh) = get_all_tasks(None, 50).await {
                        task_list.set(fresh);
                    }
                }
                Err(err) => {
                    eprintln!("Failed to set task status: {err}");
                    if let Ok(fresh) = get_all_tasks(None, 50).await {
                        task_list.set(fresh);
                    }
                }
            }
        });
    };

    let on_assign_handler = move |(task_id, user_id): (i64, i64)| {
        spawn(async move {
            if assign_task(task_id, user_id).await.is_ok() {
                if let Ok(fresh) = get_all_tasks(None, 50).await {
                    task_list.set(fresh);
                }
            }
        });
    };

    let on_unassign_handler = move |(task_id, user_id): (i64, i64)| {
        spawn(async move {
            if unassign_task(task_id, user_id).await.is_ok() {
                if let Ok(fresh) = get_all_tasks(None, 50).await {
                    task_list.set(fresh);
                }
            }
        });
    };

    let active_statuses = selected_statuses();
    let current_tasks = task_list();
    let current_users = all_users();

    rsx! {
        div { class: "task-view",
            div { class: "task-view-topbar",
                h1 { class: "task-view-title", "Task Board" }
                Link {
                    to: Route::NewTaskPage {},
                    class: "btn-create-task",
                    span { class: "btn-create-icon", "+" }
                    " New Task"
                }
            }


            // Interactive Column Selector
            ColumnSelector {
                selected_statuses: active_statuses.clone(),
                on_toggle: move |status: TaskStatus| {
                    let mut list = selected_statuses();
                    if let Some(pos) = list.iter().position(|&s| s == status) {
                        list.remove(pos);
                    } else {
                        list.push(status);
                    }
                    selected_statuses.set(list);
                },
                on_select_all: move |_| {
                    selected_statuses.set(vec![
                        TaskStatus::Suggested,
                        TaskStatus::Accepted,
                        TaskStatus::Assigned,
                        TaskStatus::InProgress,
                        TaskStatus::Finished,
                    ]);
                },
                on_clear: move |_| {
                    selected_statuses.set(Vec::new());
                },
            }

            // Columns Container: Side-by-side slideable board
            if active_statuses.is_empty() {
                div { class: "no-columns-notice",
                    p { "No columns selected. Click one or more status pills above to view tasks." }
                }
            } else {
                div { class: "task-board-scroll-container",
                    div { class: "task-columns-row",
                        for status in active_statuses {
                            TaskColumn {
                                key: "{status.as_str()}",
                                status,
                                tasks: current_tasks.clone(),
                                all_users: current_users.clone(),
                                on_dismiss: on_dismiss_handler,
                                on_status_change: on_status_change_handler,
                                on_assign: on_assign_handler,
                                on_unassign: on_unassign_handler,
                            }
                        }
                    }
                }
            }

        }
    }
}



