use dioxus::prelude::*;
use crate::types::Organization;

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
                        {
                            let is_governor = current_user_id == Some(org.governing_user_id);
                            let org_clone = org.clone();
                            rsx! {
                                div { key: "{org.id}", class: "inbox-item",
                                    div { class: "inbox-item-header",
                                        div { style: "display: flex; gap: 8px; align-items: center;",
                                            span { class: "inbox-item-title", "{org.name}" }
                                            span { class: "inbox-item-meta", "@{org.slug}" }
                                        }
                                        if is_governor {
                                            button {
                                                r#type: "button",
                                                class: "btn-action-sm",
                                                onclick: move |_| on_manage.call(org_clone.clone()),
                                                "⚙ Manage"
                                            }
                                        }
                                    }
                                    div { class: "inbox-item-msg",
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
    }
}
