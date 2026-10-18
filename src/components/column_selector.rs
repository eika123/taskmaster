use crate::types::WorkflowStatusDto;
use dioxus::prelude::*;

/// Bar allowing users to toggle column visibility and select all / clear selections.
#[component]
pub fn ColumnSelector(
    statuses: Vec<WorkflowStatusDto>,
    selected_status_ids: Vec<i64>,
    on_toggle: EventHandler<i64>,
    on_select_all: EventHandler<()>,
    on_clear: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "column-selector-bar",
            div { class: "column-selector-header",
                span { class: "column-selector-title", "Columns:" }
            }
            div { class: "column-selector-pills",
                for status in statuses {
                    {
                        let sid = status.id;
                        let is_selected = selected_status_ids.contains(&sid);
                        let name = status.name.clone();
                        rsx! {
                            ColumnStatusPill {
                                key: "{sid}",
                                status_id: sid,
                                status_name: name,
                                is_selected,
                                on_toggle: move |id| on_toggle.call(id),
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
    status_id: i64,
    status_name: String,
    is_selected: bool,
    on_toggle: EventHandler<i64>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: if is_selected { "column-pill pill-active" } else { "column-pill" },
            aria_pressed: "{is_selected}",
            onclick: move |_| on_toggle.call(status_id),
            span { class: "pill-indicator",
                if is_selected { "✓ " } else { "+ " }
            }
            "{status_name}"
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
