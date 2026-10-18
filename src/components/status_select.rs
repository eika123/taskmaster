use crate::types::WorkflowStatusDto;
use dioxus::prelude::*;

/// Dynamic status dropdown populated with the project's configured workflow statuses.
#[component]
pub fn TaskStatusSelect(
    current_status_id: i64,
    available_statuses: Vec<WorkflowStatusDto>,
    aria_label: Option<String>,
    can_mutate: bool,
    on_change: EventHandler<i64>,
) -> Element {
    let label = aria_label.unwrap_or_else(|| "Change task status".to_string());

    rsx! {
        select {
            class: "task-status-select",
            value: "{current_status_id}",
            aria_label: "{label}",
            disabled: !can_mutate,
            onchange: move |evt| {
                if can_mutate {
                    if let Ok(new_id) = evt.value().parse::<i64>() {
                        on_change.call(new_id);
                    }
                }
            },
            for s in available_statuses {
                option {
                    key: "{s.id}",
                    value: "{s.id}",
                    selected: s.id == current_status_id,
                    "{s.name}"
                }
            }
        }
    }
}
