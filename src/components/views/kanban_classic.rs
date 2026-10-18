use crate::types::{BoardSnapshot, Task, TaskMoveRequest, WorkflowStatusDto};
use crate::Route;
use dioxus::prelude::*;

/// Classic multi-column Kanban board view component.
#[component]
pub fn KanbanClassicView(
    board: ReadSignal<BoardSnapshot>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let snapshot = board();
    let statuses = snapshot.statuses.clone();

    rsx! {
        div { class: "kanban-classic-view",
            div { class: "task-board-scroll-container",
                div { class: "task-columns-row",
                    for status in statuses {
                        {
                            let col_tasks = snapshot.tasks_for_status(status.id);
                            rsx! {
                                KanbanClassicColumn {
                                    key: "{status.id}",
                                    status: status.clone(),
                                    tasks: col_tasks.into_iter().cloned().collect(),
                                    all_statuses: snapshot.statuses.clone(),
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

/// Single vertical column in the classic Kanban board.
#[component]
fn KanbanClassicColumn(
    status: WorkflowStatusDto,
    tasks: Vec<Task>,
    all_statuses: Vec<WorkflowStatusDto>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let status_id = status.id;
    let status_name = status.name.clone();
    let badge_str = status.badge_variant.as_str();
    let cat_str = status.category.as_str();
    let wip_class = if status.wip_exceeded {
        "wip-warning wip-limit-exceeded"
    } else {
        ""
    };
    let task_count = tasks.len();

    rsx! {
        div {
            class: "task-column task-column-{cat_str} badge-variant-{badge_str} {wip_class}",
            "data-status-id": "{status_id}",
            "data-wip-exceeded": "{status.wip_exceeded}",

            ClassicColumnHeader {
                status_name: status_name.clone(),
                badge_str,
                task_count,
                wip_limit: status.wip_limit,
                wip_exceeded: status.wip_exceeded,
            }

            div { class: "task-column-body",
                if tasks.is_empty() {
                    div { class: "empty-column-state",
                        p { "No {status_name} tasks" }
                    }
                } else {
                    ul { class: "task-list",
                        for task in tasks {
                            {
                                let task_id = task.id;
                                rsx! {
                                    li {
                                        key: "{task_id}",
                                        class: "task-list-item",
                                        "data-task-id": "{task_id}",
                                        ClassicTaskCard {
                                            task,
                                            current_status_id: status_id,
                                            badge_str,
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

/// Header for a Kanban column: status indicator, title, badge, count and WIP limit warning.
#[component]
fn ClassicColumnHeader(
    status_name: String,
    badge_str: &'static str,
    task_count: usize,
    wip_limit: Option<i32>,
    wip_exceeded: bool,
) -> Element {
    rsx! {
        div { class: "task-column-header",
            div { class: "task-column-title-group",
                span { class: "status-indicator status-indicator-{badge_str}" }
                h2 { class: "task-column-title", "{status_name}" }
                span { class: "badge badge-sm badge-{badge_str}", "{badge_str}" }
            }

            div { class: "task-column-meta-group",
                if let Some(limit) = wip_limit {
                    span { class: "task-column-count", "{task_count}/{limit}" }
                    if wip_exceeded {
                        span { class: "wip-badge wip-warning", "⚠️ WIP Exceeded" }
                    }
                } else {
                    span { class: "task-column-count", "{task_count}" }
                }
            }
        }
    }
}

/// Task card rendered within a Kanban column with quick status move selector.
#[component]
fn ClassicTaskCard(
    task: Task,
    current_status_id: i64,
    badge_str: &'static str,
    all_statuses: Vec<WorkflowStatusDto>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let tid = task.id;
    let title = task.title.clone();
    let desc = task.description.clone();
    let author = task.author_name.clone();
    let assignees = task.assignees.clone();

    rsx! {
        div { class: "task-card badge-variant-{badge_str}",
            div { class: "task-card-header",
                Link {
                    to: Route::TaskPage { id: tid },
                    class: "task-card-title",
                    "{title}"
                }
            }

            if !author.is_empty() && author != "Deleted User" {
                div { class: "task-card-author",
                    span { class: "author-label", "By: " }
                    span { class: "author-name", "{author}" }
                }
            }

            if !desc.is_empty() {
                p { class: "task-card-desc", "{desc}" }
            }

            div { class: "task-card-assignees",
                if assignees.is_empty() {
                    span { class: "assignee-pill unassigned", "Unassigned" }
                } else {
                    for user in assignees {
                        span { key: "{user.id}", class: "assignee-pill", "{user.name}" }
                    }
                }
            }

            div { class: "task-card-footer",
                div { class: "status-move-control",
                    label {
                        r#for: "status-select-{tid}",
                        class: "sr-only",
                        "Move status"
                    }
                    select {
                        id: "status-select-{tid}",
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
}
