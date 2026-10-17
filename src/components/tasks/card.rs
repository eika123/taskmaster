use dioxus::prelude::*;
use crate::components::assignee_search::TaskAssigneesSection;
use crate::components::status_select::TaskStatusSelect;
use crate::Route;
use crate::types::{Task, TaskStatus, User};

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
    let project_id = task.project_id;
    let project_name = task.project_name.clone();
    let title = task.title.clone();
    let author_name = task.author_name.clone();
    let description = task.description.clone();
    let current_status = task.status;
    let assignees = task.assignees.clone();

    rsx! {
        div { class: "task-card status-{current_status.as_str()}",
            div { class: "task-card-header",
                div { style: "display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0;",
                    Link {
                        to: Route::ProjectTasksPage {
                            project_id,
                        },
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

            if !author_name.is_empty() {
                div { class: "task-card-author",
                    span { class: "author-label", "Author: " }
                    span { class: "author-name", "{author_name}" }
                }
            }

            if !description.is_empty() {
                p { class: "task-card-desc", "{description}" }
            }

            TaskAssigneesSection {
                assignees: assignees.clone(),
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

