use crate::types::TaskStatus;
use dioxus::prelude::*;

/// Bar allowing users to toggle column visibility and select all / clear selections.
#[component]
pub fn ColumnSelector(
    selected_statuses: Vec<TaskStatus>,
    on_toggle: EventHandler<TaskStatus>,
    on_select_all: EventHandler<()>,
    on_clear: EventHandler<()>,
) -> Element {
    let all_statuses = [
        TaskStatus::Suggested,
        TaskStatus::Accepted,
        TaskStatus::Assigned,
        TaskStatus::InProgress,
        TaskStatus::Finished,
    ];

    rsx! {
        div { class: "column-selector-bar",
            div { class: "column-selector-header",
                span { class: "column-selector-title", "Columns:" }
            }
            div { class: "column-selector-pills",
                for status in all_statuses {
                    {
                        let is_selected = selected_statuses.contains(&status);
                        rsx! {
                            ColumnStatusPill {
                                key: "{status.as_str()}",
                                status,
                                is_selected,
                                on_toggle: move |s| on_toggle.call(s),
                            }
                        }
                    }
                }
            }
            ColumnSelectorActions {
                on_select_all: move |_| on_select_all.call(()),
                on_clear: move |_| on_clear.call(()),
            }
        }
    }
}

/// Pill toggle button for a single task column status.
#[component]
fn ColumnStatusPill(
    status: TaskStatus,
    is_selected: bool,
    on_toggle: EventHandler<TaskStatus>,
) -> Element {
    let status_label = match status {
        TaskStatus::Suggested => "Suggested",
        TaskStatus::Accepted => "Accepted",
        TaskStatus::Assigned => "Assigned",
        TaskStatus::InProgress => "In Progress",
        TaskStatus::Finished => "Finished",
    };

    rsx! {
        button {
            r#type: "button",
            class: if is_selected { "column-pill pill-active" } else { "column-pill" },
            aria_pressed: "{is_selected}",
            onclick: move |_| on_toggle.call(status),
            span { class: "pill-indicator",
                if is_selected { "✓ " } else { "+ " }
            }
            "{status_label}"
        }
    }
}

/// Action buttons ("All" and "Clear") for the column selector.
#[component]
fn ColumnSelectorActions(on_select_all: EventHandler<()>, on_clear: EventHandler<()>) -> Element {
    rsx! {
        div { class: "column-selector-actions",
            button {
                r#type: "button",
                class: "btn-pill-action",
                onclick: move |_| on_select_all.call(()),
                "All"
            }
            button {
                r#type: "button",
                class: "btn-pill-action",
                onclick: move |_| on_clear.call(()),
                "Clear"
            }
        }
    }
}
