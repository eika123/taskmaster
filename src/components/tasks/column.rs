use dioxus::prelude::*;
use super::card::TaskCard;
use crate::types::{Task, TaskStatus, User};

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
    let count = filtered_tasks.len();
    let status_name = status.to_string();

    rsx! {
        div { class: "task-column task-column-{status.as_str()}",
            div { class: "task-column-header",
                div { class: "task-column-title-group",
                    span { class: "status-indicator status-indicator-{status.as_str()}" }
                    h2 { class: "task-column-title", "{status_name}" }
                }
                span { class: "task-column-count", "{count}" }
            }
            div { class: "task-column-body",
                if filtered_tasks.is_empty() {
                    div { class: "empty-column-state",
                        p { "No {status_name} tasks" }
                    }
                } else {
                    ul { class: "task-list",
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
}

