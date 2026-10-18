use crate::types::{BoardSnapshot, Task, TaskMoveRequest, WorkflowStatusDto};
use crate::Route;
use dioxus::prelude::*;

/// Tabular task list view component.
#[component]
pub fn ListView(
    board: ReadSignal<BoardSnapshot>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let snapshot = board();
    let tasks = snapshot.tasks.clone();
    let all_statuses = snapshot.statuses.clone();

    rsx! {
        div { class: "task-list-view",
            if tasks.is_empty() {
                div { class: "empty-list-state",
                    p { "No tasks found in this project." }
                }
            } else {
                div { class: "task-table-container",
                    table { class: "task-list-table",
                        thead {
                            tr {
                                th { class: "col-id", "ID" }
                                th { class: "col-title", "Task Title" }
                                th { class: "col-status", "Status" }
                                th { class: "col-category", "Category" }
                                th { class: "col-assignees", "Assignees" }
                                th { class: "col-progress", "Progress" }
                                th { class: "col-action", "Move Status" }
                            }
                        }
                        tbody {
                            for task in tasks {
                                {
                                    let current_status = snapshot.status_for_task(&task).cloned();
                                    rsx! {
                                        TaskTableRow {
                                            key: "{task.id}",
                                            task,
                                            current_status,
                                            all_statuses: all_statuses.clone(),
                                            on_move_task,
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A single row within the task list table.
#[component]
fn TaskTableRow(
    task: Task,
    current_status: Option<WorkflowStatusDto>,
    all_statuses: Vec<WorkflowStatusDto>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let tid = task.id;
    let title = task.title.clone();
    let assignees = task.assignees.clone();
    let current_status_id = current_status.as_ref().map(|s| s.id).unwrap_or(0);
    let badge_variant_str = current_status
        .as_ref()
        .map(|s| s.badge_variant.as_str())
        .unwrap_or("default");
    let status_name = current_status
        .as_ref()
        .map(|s| s.name.clone())
        .or_else(|| task.workflow_status_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    let cat_str = current_status
        .as_ref()
        .map(|s| s.category.as_str())
        .unwrap_or("todo");

    rsx! {
        tr {
            class: "task-list-row",
            "data-task-id": "{tid}",

            td { class: "task-id-cell", "#{tid}" }
            td { class: "task-title-cell",
                Link {
                    to: Route::TaskPage { id: tid },
                    class: "task-list-title-link",
                    "{title}"
                }
            }
            td { class: "task-status-cell",
                span {
                    class: "badge badge-sm badge-{badge_variant_str} badge-variant-{badge_variant_str}",
                    "{status_name}"
                }
            }
            td { class: "task-category-cell",
                span { class: "category-tag", "{cat_str}" }
            }
            td { class: "task-assignees-cell",
                if assignees.is_empty() {
                    span { class: "assignee-pill unassigned", "Unassigned" }
                } else {
                    div { class: "assignee-pill-list",
                        for user in assignees {
                            span { key: "{user.id}", class: "assignee-pill", "{user.name}" }
                        }
                    }
                }
            }
            td { class: "task-progress-cell",
                if task.progress_percent > 0 {
                    span { class: "progress-pill", "{task.progress_percent}%" }
                } else {
                    span { style: "color: var(--text-muted);", "—" }
                }
            }
            td { class: "task-actions-cell",
                label {
                    r#for: "list-status-select-{tid}",
                    class: "sr-only",
                    "Change status for task {title}"
                }
                select {
                    id: "list-status-select-{tid}",
                    class: "task-status-select-sm",
                    value: "{current_status_id}",
                    onchange: move |evt| {
                        if let Ok(target_id) = evt.value().parse::<i64>() {
                            if target_id != current_status_id {
                                on_move_task.call(TaskMoveRequest {
                                    task_id: tid,
                                    target_workflow_status_id: target_id,
                                });
                            }
                        }
                    },
                    for opt in &all_statuses {
                        option {
                            key: "{opt.id}",
                            value: "{opt.id}",
                            selected: opt.id == current_status_id,
                            "{opt.name}"
                        }
                    }
                }
            }
        }
    }
}
