use dioxus::prelude::*;
use crate::backend::app_endpoints::suggest_task;
use crate::Route;

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

                // Header
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
                        onclick: move |_| close_modal(),
                        "✕"
                    }
                }

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

                    // Field 1: Title
                    div { class: "form-group",
                        label { r#for: "new-task-title", class: "form-label", "Task Title *" }
                        input {
                            id: "new-task-title",
                            r#type: "text",
                            class: "form-input",
                            placeholder: "e.g., Implement OAuth user authentication",
                            value: "{title}",
                            autofocus: true,
                            oninput: move |e| *title.write() = e.value(),
                        }
                    }

                    // Field 2: Description (Multi-line)
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
                            oninput: move |e| *description.write() = e.value(),
                        }
                    }

                    // Actions Footer
                    div { class: "overlay-footer",
                        button {
                            r#type: "button",
                            class: "btn-cancel",
                            onclick: move |_| close_modal(),
                            "Cancel"
                        }
                        button {
                            r#type: "submit",
                            class: "btn-submit-task",
                            disabled: is_submitting(),
                            if is_submitting() {
                                "Creating Task..."
                            } else {
                                "Create Task"
                            }
                        }
                    }
                }
            }
        }
    }
}
