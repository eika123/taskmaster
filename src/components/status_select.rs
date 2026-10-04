use dioxus::prelude::*;
use crate::backend::TaskStatus;

#[component]
pub fn TaskStatusSelect(
    status: TaskStatus,
    aria_label: Option<String>,
    on_change: Option<EventHandler<TaskStatus>>,
) -> Element {
    let label = aria_label.unwrap_or_else(|| "Change task status".to_string());

    rsx! {
        select {
            class: "task-status-select",
            value: "{status.as_str()}",
            aria_label: "{label}",
            onchange: move |evt| {
                if let Ok(new_status) = evt.value().parse::<TaskStatus>() {
                    if let Some(cb) = on_change {
                        cb.call(new_status);
                    }
                }
            },
            option {
                value: "suggested",
                selected: status == TaskStatus::Suggested,
                "Suggested"
            }
            option {
                value: "accepted",
                selected: status == TaskStatus::Accepted,
                "Accepted"
            }
            option {
                value: "assigned",
                selected: status == TaskStatus::Assigned,
                "Assigned"
            }
            option {
                value: "in_progress",
                selected: status == TaskStatus::InProgress,
                "In Progress"
            }
            option {
                value: "finished",
                selected: status == TaskStatus::Finished,
                "Finished"
            }
        }
    }
}
