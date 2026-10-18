use crate::types::Organization;
use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn DashboardOrganizations(
    organizations: Vec<Organization>,
    on_manage: EventHandler<Organization>,
) -> Element {
    let auth = use_context::<crate::components::AuthState>();
    let current_user_id = auth.user.read().as_ref().map(|u| u.id);

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
                        OrganizationDashboardRow {
                            key: "{org.id}",
                            org,
                            current_user_id,
                        }
                    }
                }
            }
        }
    }
}

/// Item representing a single organization on the user's dashboard.
#[component]
fn OrganizationDashboardRow(org: Organization, current_user_id: Option<i64>) -> Element {
    let is_governor = current_user_id == Some(org.governing_user_id);
    let oid = org.id;

    rsx! {
        div { class: "inbox-item",
            div { class: "inbox-item-header",
                div { style: "display: flex; gap: 8px; align-items: center;",
                    Link {
                        to: Route::OrganizationDashboardPage { organization_id: oid },
                        class: "inbox-item-title",
                        style: "text-decoration: none; color: inherit; font-weight: var(--font-weight-bold);",
                        "{org.name}"
                    }
                    span { class: "inbox-item-meta", "@{org.slug}" }
                }
                Link {
                    to: Route::OrganizationDashboardPage { organization_id: oid },
                    class: "btn-action-sm",
                    style: "text-decoration: none;",
                    if is_governor { "⚙ Manage" } else { "View Dashboard →" }
                }
            }
            div { class: "inbox-item-msg",
                "Governed by: "
                strong { "{org.governing_user_name}" }
            }
        }
    }
}
