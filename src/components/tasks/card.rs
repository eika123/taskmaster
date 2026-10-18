use crate::components::assignee_search::TaskAssigneesSection;
use crate::components::status_select::TaskStatusSelect;
use crate::types::{Task, TaskStatus, User};
use crate::Route;
use dioxus::prelude::*;

/// Task card rendered in legacy TaskColumn view.
#[component]
pub fn TaskCard(
    task: Task,
    all_users: Vec<User>,
    can_mutate: bool,
    on_dismiss: EventHandler<i64>,
    on_status_change: EventHandler<(u64, TaskStatus)>,
    on_assign: EventHandler<(i64, i64)>,
    on_unassign: EventHandler<(i64, i64)>,
) -> Element {
    let task_id = task.id;
    let title = task.title.clone();
    let current_status = task.status;

    rsx! {
        div { class: "task-card status-{current_status.as_str()}",
            CardTopSection {
                task_id,
                project_id: task.project_id,
                project_name: task.project_name.clone(),
                title: title.clone(),
                can_mutate,
                on_dismiss,
            }

            CardAuthorAndDesc {
                author_name: task.author_name.clone(),
                description: task.description.clone(),
            }

            TaskAssigneesSection {
                assignees: task.assignees.clone(),
                all_users,
                task_title: title.clone(),
                can_mutate,
                on_assign: move |uid| on_assign.call((task_id, uid)),
                on_unassign: move |uid| on_unassign.call((task_id, uid)),
            }

            div { class: "task-card-footer",
                TaskStatusSelect {
                    status: current_status,
                    aria_label: format!("Status for task: {title}"),
                    can_mutate,
                    on_change: move |new_st| on_status_change.call((task_id as u64, new_st)),
                }
            }
        }
    }
}

/// Top header of the task card with project badge, title link, and dismiss button.
#[component]
fn CardTopSection(
    task_id: i64,
    project_id: i64,
    project_name: String,
    title: String,
    can_mutate: bool,
    on_dismiss: EventHandler<i64>,
) -> Element {
    rsx! {
        div { class: "task-card-header",
            div { style: "display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0;",
                Link {
                    to: Route::ProjectTasksPage { project_id },
                    class: "task-card-project-badge",
                    "📁 {project_name}"
                }
                Link {
                    to: Route::TaskPage { id: task_id },
                    class: "task-card-title",
                    "{title}"
                }
            }
            if can_mutate {
                button {
                    r#type: "button",
                    class: "btn-dismiss",
                    aria_label: "Dismiss task: {title}",
                    onclick: move |_| on_dismiss.call(task_id),
                    "✕"
                }
            }
        }
    }
}

/// Author label and description snippet of the task card.
#[component]
fn CardAuthorAndDesc(author_name: String, description: String) -> Element {
    rsx! {
        if !author_name.is_empty() && author_name != "Deleted User" {
            div { class: "task-card-author",
                span { class: "author-label", "Author: " }
                span { class: "author-name", "{author_name}" }
            }
        }
        if !description.is_empty() {
            p { class: "task-card-desc", "{description}" }
        }
    }
}
