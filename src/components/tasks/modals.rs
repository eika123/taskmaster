use crate::backend::app_endpoints::suggest_task_for_project;
use dioxus::prelude::*;

/// Inline modal for creating a task directly on project board or My Tasks page.
#[component]
pub fn InlineCreateTaskModal(
    project_id: Option<i64>,
    on_close: EventHandler<()>,
    on_created: EventHandler<()>,
) -> Element {
    let mut new_task_title = use_signal(String::new);
    let mut new_task_desc = use_signal(String::new);
    let mut new_task_err = use_signal(|| Option::<String>::None);
    let mut is_creating_task = use_signal(|| false);

    let handle_create_task = move |_| {
        let title = new_task_title.read().trim().to_string();
        let desc = new_task_desc.read().trim().to_string();
        if title.is_empty() {
            new_task_err.set(Some("Task title is required.".to_string()));
            return;
        }

        is_creating_task.set(true);
        new_task_err.set(None);

        // Fallback to 0 lets backend auto-resolve the user's active project instead of failing on hardcoded ID
        let target_pid = project_id.unwrap_or(0);
        spawn(async move {
            match suggest_task_for_project(target_pid, title, desc).await {
                Ok(_) => {
                    new_task_title.write().clear();
                    new_task_desc.write().clear();
                    is_creating_task.set(false);
                    on_created.call(());
                }
                Err(err) => {
                    new_task_err.set(Some(err.to_string()));
                    is_creating_task.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "dashboard-panel", style: "border: 1px solid var(--primary); margin-bottom: 20px;",
            h2 { class: "dashboard-panel-title", "Create a Task" }
            if let Some(err) = &*new_task_err.read() {
                div { class: "task-detail-error", "{err}" }
            }
            div { style: "display: flex; flex-direction: column; gap: 12px;",
                InlineTaskInputs {
                    title: new_task_title,
                    desc: new_task_desc,
                }
                InlineTaskActions {
                    is_creating: *is_creating_task.read(),
                    on_close,
                    on_submit: handle_create_task,
                }
            }
        }
    }
}

/// Text input and textarea for task title and description.
#[component]
fn InlineTaskInputs(mut title: Signal<String>, mut desc: Signal<String>) -> Element {
    rsx! {
        input {
            class: "task-edit-title-input",
            placeholder: "Task Title (e.g. Implement OAuth Flow)",
            value: "{title}",
            oninput: move |e| title.set(e.value()),
        }
        textarea {
            class: "task-edit-description-textarea",
            placeholder: "Describe requirements, criteria, or context...",
            value: "{desc}",
            oninput: move |e| desc.set(e.value()),
        }
    }
}

/// Action buttons for the inline task creation form.
#[component]
fn InlineTaskActions(
    is_creating: bool,
    on_close: EventHandler<()>,
    on_submit: EventHandler<()>,
) -> Element {
    rsx! {
        div { style: "display: flex; gap: 8px; justify-content: flex-end;",
            button {
                r#type: "button",
                class: "btn-task-cancel",
                onclick: move |_| on_close.call(()),
                "Cancel"
            }
            button {
                r#type: "button",
                class: "btn-task-save",
                disabled: is_creating,
                onclick: move |_| on_submit.call(()),
                if is_creating { "Creating..." } else { "Create Task" }
            }
        }
    }
}
