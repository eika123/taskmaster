use super::card::TaskCard;
use crate::types::{Task, User, WorkflowStatusDto};
use dioxus::prelude::*;

/// Kanban column displaying tasks with a given workflow status.
#[component]
pub fn TaskColumn(
    status: WorkflowStatusDto,
    tasks: Vec<Task>,
    all_statuses: Vec<WorkflowStatusDto>,
    all_users: Vec<User>,
    can_mutate: bool,
    on_dismiss: EventHandler<i64>,
    on_status_change: EventHandler<(i64, i64)>,
    on_assign: EventHandler<(i64, i64)>,
    on_unassign: EventHandler<(i64, i64)>,
) -> Element {
    let sid = status.id;
    let filtered_tasks: Vec<Task> = tasks
        .into_iter()
        .filter(|t| t.workflow_status_id == sid)
        .collect();
    let count = filtered_tasks.len();
    let status_name = status.name.clone();
    let cat_str = status.category.as_str();
    let badge_str = status.badge_variant.as_str();
    let is_wip_exceeded = status.wip_limit.is_some_and(|lim| count > lim as usize);

    rsx! {
        div { class: "task-column task-column-{cat_str} badge-variant-{badge_str}",
            TaskColumnHeader {
                variant: badge_str.to_string(),
                status_name: status_name.clone(),
                count,
                wip_limit: status.wip_limit,
                is_wip_exceeded,
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
                                    all_statuses: all_statuses.clone(),
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

/// Header for a Kanban task column showing title indicator, task count, and soft WIP warnings.
#[component]
fn TaskColumnHeader(
    variant: String,
    status_name: String,
    count: usize,
    wip_limit: Option<i32>,
    is_wip_exceeded: bool,
) -> Element {
    rsx! {
        div { class: "task-column-header",
            div { class: "task-column-title-group",
                span { class: "status-indicator status-indicator-{variant}" }
                h2 { class: "task-column-title", "{status_name}" }
                if let Some(lim) = wip_limit {
                    span {
                        class: if is_wip_exceeded { "wip-badge wip-warning" } else { "wip-badge" },
                        style: "font-size: var(--font-size-meta); padding: 2px 6px; border-radius: var(--radius-sm);",
                        "WIP {count}/{lim}"
                    }
                }
            }
            span { class: "task-column-count", "{count}" }
        }
    }
}
