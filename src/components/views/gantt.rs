use crate::types::{BoardSnapshot, Task, WorkflowStatusDto};
use crate::Route;
use dioxus::prelude::*;

/// Gantt chart and project timeline component.
#[component]
pub fn GanttView(board: ReadSignal<BoardSnapshot>) -> Element {
    let snapshot = board();
    let tasks = snapshot.tasks.clone();
    let dep_count = snapshot.dependencies.len();

    let total_tasks = tasks.len();
    let scheduled_tasks = tasks
        .iter()
        .filter(|t| t.start_date.is_some() || t.due_date.is_some())
        .count();
    let total_hours: i32 = tasks.iter().filter_map(|t| t.estimated_hours).sum();

    rsx! {
        div { class: "gantt-view",
            GanttSummaryCards {
                total_tasks,
                scheduled_tasks,
                total_hours,
                dep_count,
            }

            if tasks.is_empty() {
                div { class: "empty-gantt-state",
                    p { "No tasks scheduled for timeline view." }
                }
            } else {
                div { class: "gantt-timeline-container",
                    table { class: "gantt-table",
                        thead {
                            tr {
                                th { class: "gantt-col-id", "ID" }
                                th { class: "gantt-col-title", "Task Title" }
                                th { class: "gantt-col-status", "Status" }
                                th { class: "gantt-col-dates", "Timeline Window" }
                                th { class: "gantt-col-hours", "Est. Hours" }
                                th { class: "gantt-col-progress", "Progress" }
                            }
                        }
                        tbody {
                            for task in tasks {
                                {
                                    let current_status = snapshot.status_for_task(&task).cloned();
                                    rsx! {
                                        GanttTaskRow {
                                            key: "{task.id}",
                                            task,
                                            current_status,
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

/// Metric summary cards above the Gantt chart.
#[component]
fn GanttSummaryCards(
    total_tasks: usize,
    scheduled_tasks: usize,
    total_hours: i32,
    dep_count: usize,
) -> Element {
    rsx! {
        div { class: "gantt-header-summary",
            div { class: "summary-card",
                span { class: "summary-label", "Total Tasks" }
                span { class: "summary-val", "{total_tasks}" }
            }
            div { class: "summary-card",
                span { class: "summary-label", "Scheduled" }
                span { class: "summary-val", "{scheduled_tasks}" }
            }
            div { class: "summary-card",
                span { class: "summary-label", "Estimated Hours" }
                span { class: "summary-val", "{total_hours}h" }
            }
            div { class: "summary-card",
                span { class: "summary-label", "Dependencies" }
                span { class: "summary-val", "{dep_count}" }
            }
        }
    }
}

/// Individual timeline row for a task in the Gantt table.
#[component]
fn GanttTaskRow(task: Task, current_status: Option<WorkflowStatusDto>) -> Element {
    let tid = task.id;
    let title = task.title.clone();
    let badge_variant_str = current_status
        .as_ref()
        .map(|s| s.badge_variant.as_str())
        .unwrap_or("default");
    let status_name = current_status
        .as_ref()
        .map(|s| s.name.clone())
        .or_else(|| task.workflow_status_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    let start_str = task
        .start_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "Unset".to_string());
    let due_str = task
        .due_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "Unset".to_string());
    let hours_str = task
        .estimated_hours
        .map(|h| format!("{h}h"))
        .unwrap_or_else(|| "—".to_string());
    let pct = task.progress_percent;

    rsx! {
        tr {
            class: "gantt-row",
            "data-task-id": "{tid}",

            td { class: "gantt-cell-id", "#{tid}" }
            td { class: "gantt-cell-title",
                Link {
                    to: Route::TaskPage { id: tid },
                    class: "task-list-title-link",
                    "{title}"
                }
            }
            td { class: "gantt-cell-status",
                span {
                    class: "badge badge-sm badge-{badge_variant_str} badge-variant-{badge_variant_str}",
                    "{status_name}"
                }
            }
            td { class: "gantt-cell-dates",
                div { class: "gantt-date-range",
                    span { class: "date-start", "{start_str}" }
                    span { class: "date-arrow", " → " }
                    span { class: "date-due", "{due_str}" }
                }
            }
            td { class: "gantt-cell-hours", "{hours_str}" }
            td { class: "gantt-cell-progress",
                div { class: "gantt-progress-wrapper",
                    div { class: "gantt-progress-track",
                        div {
                            class: "gantt-progress-fill",
                            style: "width: {pct}%;"
                        }
                    }
                    span { class: "gantt-progress-label", "{pct}%" }
                }
            }
        }
    }
}
