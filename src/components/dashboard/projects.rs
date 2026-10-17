use dioxus::prelude::*;
use crate::types::Project;
use crate::Route;

#[component]
pub fn DashboardProjects(
    projects: Vec<Project>,
    on_invite: EventHandler<(i64, String)>,
) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            h2 { class: "dashboard-panel-title",
                span { "My Projects ({projects.len()})" }
            }

            if projects.is_empty() {
                p { class: "empty-description", "You are not a member of any projects yet. Create one above!" }
            } else {
                div { class: "project-card-list",
                    for proj in projects {
                        {
                            let pid = proj.id;
                            let pname = proj.name.clone();
                            let pname_invite = proj.name.clone();
                            rsx! {
                                div { key: "{pid}", class: "project-card",
                                    div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                                        Link {
                                            to: Route::ProjectTasksPage { project_id: pid },
                                            class: "project-card-title",
                                            "{pname}"
                                        }
                                        if let Some(org_name) = &proj.owner_org_name {
                                            span { class: "task-detail-status status-accepted", style: "font-size: 0.72rem; padding: 2px 8px;",
                                                "{org_name}"
                                            }
                                        } else {
                                            span { class: "task-detail-status status-suggested", style: "font-size: 0.72rem; padding: 2px 8px;",
                                                "Standalone"
                                            }
                                        }
                                    }
                                    p { class: "project-card-desc",
                                        if proj.description.is_empty() { "No description." } else { "{proj.description}" }
                                    }
                                    div { class: "project-card-meta",
                                        span { "Leader: {proj.leader_name}" }
                                        div { style: "display: flex; gap: 8px;",
                                            button {
                                                r#type: "button",
                                                style: "background: transparent; border: 1px solid var(--border-default); color: var(--text-secondary); padding: 2px 8px; border-radius: var(--radius-sm); font-size: 0.75rem; cursor: pointer;",
                                                onclick: move |_| {
                                                    on_invite.call((pid, pname_invite.clone()));
                                                },
                                                "+ Invite"
                                            }
                                            Link {
                                                to: Route::ProjectTasksPage { project_id: pid },
                                                style: "color: var(--primary); text-decoration: none; font-weight: 600;",
                                                "View Board →"
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
    }
}

