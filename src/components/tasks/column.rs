use super::card::TaskCard;
use crate::types::{Task, TaskStatus, User};
use dioxus::prelude::*;

/// Kanban column displaying tasks with a given status.
#[component]
pub fn TaskColumn(
    status: TaskStatus,
    tasks: Vec<Task>,
    all_users: Vec<User>,
    can_mutate: bool,
    on_dismiss: EventHandler<i64>,
    on_status_change: EventHandler<(u64, TaskStatus)>,
    on_assign: EventHandler<(i64, i64)>,
    on_unassign: EventHandler<(i64, i64)>,
) -> Element {
    let filtered_tasks: Vec<Task> = tasks.into_iter().filter(|t| t.status == status).collect();
    let count = filtered_tasks.len();
    let status_name = status.to_string();

    rsx! {
        div { class: "task-column task-column-{status.as_str()}",
            TaskColumnHeader {
                status,
                status_name: status_name.clone(),
                count,
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
                                    can_mutate,
                                    on_dismiss: move |id| on_dismiss.call(id),
                                    on_status_change: move |args| on_status_change.call(args),
                                    on_assign: move |args| on_assign.call(args),
                                    on_unassign: move |args| on_unassign.call(args),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Header for a Kanban task column showing title indicator and task count.
#[component]
fn TaskColumnHeader(status: TaskStatus, status_name: String, count: usize) -> Element {
    rsx! {
        div { class: "task-column-header",
            div { class: "task-column-title-group",
                span { class: "status-indicator status-indicator-{status.as_str()}" }
                h2 { class: "task-column-title", "{status_name}" }
            }
            span { class: "task-column-count", "{count}" }
        }
    }
}
