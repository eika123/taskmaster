use dioxus::prelude::*;
use crate::components::assignee_search::TaskAssigneesSection;
use crate::components::status_select::TaskStatusSelect;
use crate::Route;
use crate::types::{Task, TaskStatus, User};

#[component]
pub fn TaskCard(
    task: Task,
    all_users: Vec<User>,
    on_dismiss: Option<EventHandler<i64>>,
    on_status_change: Option<EventHandler<(u64, TaskStatus)>>,
    on_assign: Option<EventHandler<(i64, i64)>>,
    on_unassign: Option<EventHandler<(i64, i64)>>,
) -> Element {
    let task_id = task.id;
    let title = task.title.clone();
    let author_name = task.author_name.clone();
    let description = task.description.clone();
    let current_status = task.status;
    let assignees = task.assignees.clone();

    let dismiss_cb = on_dismiss;
    let status_cb = on_status_change;
    let assign_cb = on_assign;
    let unassign_cb = on_unassign;

    rsx! {
        div { class: "task-card status-{current_status.as_str()}",
            div { class: "task-card-header",
                Link {
                    to: Route::TaskPage { id: task_id },
                    class: "task-card-title",
                    "{title}"
                }
                if let Some(cb) = dismiss_cb {
                    button {
                        r#type: "button",
                        class: "btn-dismiss",
                        aria_label: "Dismiss task: {title}",
                        onclick: move |_| cb.call(task_id),
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
                on_assign: move |uid| {
                    if let Some(cb) = assign_cb {
                        cb.call((task_id, uid));
                    }
                },
                on_unassign: move |uid| {
                    if let Some(cb) = unassign_cb {
                        cb.call((task_id, uid));
                    }
                },
            }

            div { class: "task-card-footer",
                TaskStatusSelect {
                    status: current_status,
                    aria_label: format!("Status for task: {title}"),
                    on_change: move |new_st| {
                        if let Some(cb) = status_cb {
                            cb.call((task_id as u64, new_st));
                        }
                    },
                }
            }
        }
    }
}

