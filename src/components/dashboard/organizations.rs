use dioxus::prelude::*;
use crate::types::Organization;

#[component]
pub fn DashboardOrganizations(
    organizations: Vec<Organization>,
) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            h2 { class: "dashboard-panel-title",
                span { "My Organizations ({organizations.len()})" }
            }

            if organizations.is_empty() {
                p { class: "empty-description", "You do not belong to any organizations yet." }
            } else {
                div { style: "display: flex; flex-direction: column; gap: 12px;",
                    for org in organizations {
                        div { key: "{org.id}", class: "inbox-item",
                            div { class: "inbox-item-header",
                                span { class: "inbox-item-title", "{org.name}" }
                                span { class: "inbox-item-meta", "@{org.slug}" }
                            }
                            div { style: "font-size: 0.85rem; color: var(--text-secondary);",
                                "Governed by: "
                                strong { "{org.governing_user_name}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

