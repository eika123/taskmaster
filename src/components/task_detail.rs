use dioxus::prelude::*;
use crate::backend::get_task;
use crate::Route;

#[component]
pub fn TaskPage(id: i64) -> Element {
    let task_resource = use_server_future(move || async move {
        get_task(id).await
    })?;

    let task_res = task_resource.read();

    rsx! {
        div { class: "task-detail-container",
            div { class: "task-detail-nav",
                Link { to: Route::TasksDashboardPage {}, class: "btn-back-link",
                    "← Back to Task Board"
                }
            }

            match &*task_res {
                Some(Ok(task)) => rsx! {
                    div { class: "task-detail-card",
                        div { class: "task-detail-header",
                            div { class: "task-detail-title-group",
                                h1 { class: "task-detail-title", "{task.title}" }
                                span { class: "task-detail-meta",
                                    "Task #{task.id} • Created by "
                                    strong { "{task.author_name}" }
                                }
                            }
                            span { class: "task-detail-status status-{task.status.as_str()}", "{task.status}" }
                        }

                        div { class: "task-detail-section",
                            h2 { class: "section-title", "Description" }
                            if task.description.is_empty() {
                                p { class: "empty-description", "No description provided." }
                            } else {
                                p { class: "task-detail-description-text", "{task.description}" }
                            }
                        }

                        div { class: "task-detail-section",
                            h2 { class: "section-title", "Assignees" }
                            if task.assignees.is_empty() {
                                p { class: "empty-assignees", "No assignees on this task." }
                            } else {
                                ul { class: "detail-assignees-list",
                                    for user in &task.assignees {
                                        li { key: "{user.id}", class: "detail-assignee-item",
                                            span { class: "detail-assignee-name", "{user.name}" }
                                            span { class: "detail-assignee-email", "({user.email})" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(err)) => rsx! {
                    div { class: "task-detail-error",
                        h2 { "Task Not Found" }
                        p { "Could not load task #{id}: {err}" }
                    }
                },
                None => rsx! {
                    div { class: "task-detail-loading",
                        p { "Loading task details..." }
                    }
                },
            }
        }
    }
}

