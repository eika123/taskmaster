use crate::backend::app_endpoints::suggest_task;
use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn NewTaskPage() -> Element {
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut is_submitting = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    let nav = use_navigator();

    let close_modal = move || {
        nav.push(Route::TasksDashboardPage {});
    };

    rsx! {
        div {
            class: "overlay-backdrop",
            onclick: move |_| close_modal(),
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    close_modal();
                }
            },
            div {
                class: "overlay-modal",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "new-task-heading",
                onclick: move |e| e.stop_propagation(),

                NewTaskHeader { on_close: move |_| close_modal() }

                // Form Body
                form {
                    class: "new-task-form",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let current_title = title.read().trim().to_string();
                        let current_desc = description.read().trim().to_string();

                        if current_title.is_empty() {
                            *error_msg.write() = Some("Task title is required.".to_string());
                            return;
                        }

                        *error_msg.write() = None;
                        *is_submitting.write() = true;

                        spawn(async move {
                            match suggest_task(current_title, current_desc).await {
                                Ok(_) => {
                                    nav.push(Route::TasksDashboardPage {});
                                }

                                Err(err) => {
                                    *error_msg.write() = Some(format!("Failed to create task: {err}"));
                                    *is_submitting.write() = false;
                                }
                            }
                        });
                    },

                    if let Some(err) = error_msg() {
                        div { class: "form-error-banner", "{err}" }
                    }

                    NewTaskFormFields {
                        title: title.read().clone(),
                        description: description.read().clone(),
                        on_title_change: move |val| *title.write() = val,
                        on_desc_change: move |val| *description.write() = val,
                    }

                    NewTaskActions {
                        is_submitting: is_submitting(),
                        on_cancel: move |_| close_modal(),
                    }
                }
            }
        }
    }
}

/// Header with title and close button for the new task modal dialog.
#[component]
fn NewTaskHeader(on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "overlay-header",
            div { class: "overlay-title-group",
                h2 { id: "new-task-heading", class: "overlay-title", "Create New Task" }
                p { class: "overlay-subtitle",
                    "Add a new task with details, requirements, and context."
                }
            }
            button {
                r#type: "button",
                class: "btn-close-overlay",
                aria_label: "Close modal",
                onclick: move |_| on_close.call(()),
                "✕"
            }
        }
    }
}

/// Text input and textarea fields for task title and description.
#[component]
fn NewTaskFormFields(
    title: String,
    description: String,
    on_title_change: EventHandler<String>,
    on_desc_change: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "form-group",
            label { r#for: "new-task-title", class: "form-label", "Task Title *" }
            input {
                id: "new-task-title",
                r#type: "text",
                class: "form-input",
                placeholder: "e.g., Implement OAuth user authentication",
                value: "{title}",
                autofocus: true,
                oninput: move |e| on_title_change.call(e.value()),
            }
        }

        div { class: "form-group",
            label { r#for: "new-task-desc", class: "form-label",
                "Description & Specifications"
            }
            textarea {
                id: "new-task-desc",
                class: "form-textarea",
                rows: 6,
                placeholder: "Describe the task goals, acceptance criteria, or relevant technical details...",
                value: "{description}",
                oninput: move |e| on_desc_change.call(e.value()),
            }
        }
    }
}

/// Cancel and submit actions for the task creation form.
#[component]
fn NewTaskActions(is_submitting: bool, on_cancel: EventHandler<()>) -> Element {
    rsx! {
        div { class: "overlay-footer",
            button {
                r#type: "button",
                class: "btn-cancel",
                onclick: move |_| on_cancel.call(()),
                "Cancel"
            }
            button {
                r#type: "submit",
                class: "btn-submit-task",
                disabled: is_submitting,
                if is_submitting {
                    "Creating Task..."
                } else {
                    "Create Task"
                }
            }
        }
    }
}
