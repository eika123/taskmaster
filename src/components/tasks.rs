use dioxus::prelude::*;
use crate::backend::{delete_task, get_all_tasks, suggest_task, Task};


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
pub fn SuggestedTaskCard(
    task: Task,
    on_dismiss: Option<EventHandler<i64>>,
) -> Element {
    let task_id = task.id;
    let title = task.title.clone();
    let description = task.description.clone();

    rsx! {
        div { class: "task-card task-card-suggested",
            div { class: "task-card-content",
                div { class: "task-card-header",
                    h3 { class: "task-card-title", "{title}" }
                }
                if !description.is_empty() {
                    div { class: "task-card-body",
                        p { class: "task-card-description", "{description}" }
                    }
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

#[component]
pub fn SuggestedTasks(
    tasks: Vec<Task>,
    on_dismiss: Option<EventHandler<i64>>,
) -> Element {
    rsx! {
        div { id: "task-status-suggested", class: "task-status-group",
            h2 { "Suggested Tasks" }
            if tasks.is_empty() {
                p { class: "empty-message", "No suggested tasks present" }
            } else {
                ul { class: "task-list", role: "list",
                    for task in tasks {
                        li { key: "{task.id}", class: "task-list-item",
                            SuggestedTaskCard {
                                task: task.clone(),
                                on_dismiss: move |id| {
                                    if let Some(cb) = on_dismiss {
                                        cb.call(id);
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

#[component]
pub fn TaskView() -> Element {
    let mut tasks = use_server_future(|| async move {
        get_all_tasks(None, 10).await
    })?;

    let res = tasks.read();
    let task_list = match &*res {
        Some(Ok(list)) => list.clone(),
        Some(Err(err)) => return rsx! {
            p { "Failed to load tasks: {err}" }
        },
        None => return rsx! {
            p { "Loading tasks..." }
        },
    };

    rsx! {
        div { class: "task-view",
            SuggestTaskForm {
                on_submit: move |data: TaskData| {
                    spawn(async move {
                        if suggest_task(
                                "dummy@example.com".to_string(),
                                data.title,
                                data.description,
                            )
                            .await
                            .is_ok()
                        {
                            tasks.restart();
                        }
                    });
                },
            }
            SuggestedTasks {
                tasks: task_list,
                on_dismiss: move |task_id: i64| {
                    spawn(async move {
                        if delete_task(task_id).await.is_ok() {
                            tasks.restart();
                        }
                    });
                },
            }
        }
    }
}


