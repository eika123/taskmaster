use crate::types::{BoardSnapshot, Task, TaskMoveRequest, User, WorkflowStatusDto};
use crate::Route;
use dioxus::prelude::*;

/// Kanban swimlanes view component.
#[component]
pub fn KanbanSwimlanesView(
    board: ReadSignal<BoardSnapshot>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let snapshot = board();
    let statuses = snapshot.statuses.clone();
    let lane_map = build_swimlane_groups(&snapshot);

    rsx! {
        div { class: "kanban-swimlanes-view",
            div { class: "swimlanes-container",
                for (maybe_user, lane_tasks) in lane_map {
                    {
                        let lane_title = maybe_user
                            .as_ref()
                            .map(|u| u.name.clone())
                            .unwrap_or_else(|| "Unassigned".to_string());
                        let lane_key = maybe_user
                            .as_ref()
                            .map(|u| format!("user-{}", u.id))
                            .unwrap_or_else(|| "unassigned".to_string());
                        let is_user = maybe_user.is_some();

                        rsx! {
                            SwimlaneRow {
                                key: "{lane_key}",
                                lane_key,
                                lane_title,
                                is_user,
                                lane_tasks,
                                statuses: statuses.clone(),
                                board,
                                on_move_task,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Helper function to partition tasks into distinct assignee swimlane groups.
fn build_swimlane_groups(snapshot: &BoardSnapshot) -> Vec<(Option<User>, Vec<Task>)> {
    let mut lane_map: Vec<(Option<User>, Vec<Task>)> = Vec::new();
    let unassigned_tasks: Vec<Task> = snapshot
        .tasks
        .iter()
        .filter(|t| t.assignees.is_empty())
        .cloned()
        .collect();

    let mut distinct_users: Vec<User> = Vec::new();
    for task in &snapshot.tasks {
        for u in &task.assignees {
            if !distinct_users.iter().any(|existing| existing.id == u.id) {
                distinct_users.push(u.clone());
            }
        }
    }
    distinct_users.sort_by(|a, b| a.name.cmp(&b.name));

    for user in distinct_users {
        let user_id = user.id;
        let user_tasks: Vec<Task> = snapshot
            .tasks
            .iter()
            .filter(|t| t.assignees.first().map(|a| a.id) == Some(user_id))
            .cloned()
            .collect();
        lane_map.push((Some(user), user_tasks));
    }

    if !unassigned_tasks.is_empty() || lane_map.is_empty() {
        lane_map.insert(0, (None, unassigned_tasks));
    }
    lane_map
}

/// A horizontal swimlane row representing an assignee (or unassigned tasks).
#[component]
fn SwimlaneRow(
    lane_key: String,
    lane_title: String,
    is_user: bool,
    lane_tasks: Vec<Task>,
    statuses: Vec<WorkflowStatusDto>,
    board: ReadSignal<BoardSnapshot>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let lane_count = lane_tasks.len();
    let snapshot = board();

    rsx! {
        div {
            class: "swimlane-row",
            "data-lane": "{lane_key}",

            div { class: "swimlane-header",
                div { class: "swimlane-header-left",
                    span { class: "swimlane-icon", if is_user { "👤" } else { "📋" } }
                    h3 { class: "swimlane-title", "{lane_title}" }
                }
                span { class: "swimlane-count", "{lane_count} tasks" }
            }

            div { class: "swimlane-columns-grid",
                for status in &statuses {
                    {
                        let status_id = status.id;
                        let cell_tasks: Vec<Task> = lane_tasks
                            .iter()
                            .filter(|t| snapshot.status_for_task(t).map(|s| s.id) == Some(status_id))
                            .cloned()
                            .collect();

                        rsx! {
                            SwimlaneCell {
                                key: "{status_id}",
                                status: status.clone(),
                                cell_tasks,
                                all_statuses: statuses.clone(),
                                on_move_task,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Intersection cell of an assignee swimlane and a vertical workflow status column.
#[component]
fn SwimlaneCell(
    status: WorkflowStatusDto,
    cell_tasks: Vec<Task>,
    all_statuses: Vec<WorkflowStatusDto>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let status_id = status.id;
    let status_name = status.name.clone();
    let badge_str = status.badge_variant.as_str();
    let cat_str = status.category.as_str();
    let wip_class = if status.wip_exceeded {
        "wip-warning"
    } else {
        ""
    };
    let cell_count = cell_tasks.len();

    rsx! {
        div {
            class: "swimlane-cell task-column-{cat_str} badge-variant-{badge_str} {wip_class}",
            "data-status-id": "{status_id}",

            div { class: "swimlane-cell-header",
                span { class: "swimlane-cell-status-name", "{status_name}" }
                span { class: "swimlane-cell-count", "{cell_count}" }
            }

            div { class: "swimlane-cell-body",
                if cell_tasks.is_empty() {
                    div { class: "swimlane-cell-empty", span { "—" } }
                } else {
                    for task in cell_tasks {
                        SwimlaneCard {
                            key: "{task.id}",
                            task,
                            status_id,
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

/// Compact card rendered within a swimlane cell.
#[component]
fn SwimlaneCard(
    task: Task,
    status_id: i64,
    badge_str: &'static str,
    all_statuses: Vec<WorkflowStatusDto>,
    on_move_task: EventHandler<TaskMoveRequest>,
) -> Element {
    let tid = task.id;
    let title = task.title.clone();

    rsx! {
        div {
            class: "swimlane-card task-card badge-variant-{badge_str}",
            "data-task-id": "{tid}",

            div { class: "swimlane-card-title",
                Link {
                    to: Route::TaskPage { id: tid },
                    class: "task-card-title",
                    "{title}"
                }
            }

            div { class: "swimlane-card-footer",
                select {
                    class: "task-status-select-sm",
                    value: "{status_id}",
                    onchange: move |evt| {
                        if let Ok(target_id) = evt.value().parse::<i64>() {
                            if target_id != status_id {
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
                            selected: opt.id == status_id,
                            "{opt.name}"
                        }
                    }
                }
            }
        }
    }
}
