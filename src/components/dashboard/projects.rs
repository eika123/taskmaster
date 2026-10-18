use crate::types::Project;
use crate::Route;
use dioxus::prelude::*;

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
                        ProjectDashboardCard {
                            key: "{proj.id}",
                            proj,
                            on_invite: move |inv| on_invite.call(inv),
                        }
                    }
                }
            }
        }
    }
}

/// Card showing project metadata, status, leader, and action links.
#[component]
fn ProjectDashboardCard(proj: Project, on_invite: EventHandler<(i64, String)>) -> Element {
    let pid = proj.id;
    let pname = proj.name.clone();
    let pname_invite = proj.name.clone();

    rsx! {
        div { class: "project-card",
            div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                Link {
                    to: Route::ProjectTasksPage { project_id: pid },
                    class: "project-card-title",
                    "{pname}"
                }
                if let Some(org_name) = &proj.owner_org_name {
                    span { class: "task-detail-status status-accepted badge-sm",
                        "{org_name}"
                    }
                } else {
                    span { class: "task-detail-status status-suggested badge-sm",
                        "Standalone"
                    }
                }
            }
            p { class: "project-card-desc",
                if proj.description.is_empty() { "No description." } else { "{proj.description}" }
            }
            div { class: "project-card-meta",
                span { "Leader: {proj.leader_name}" }
                div { style: "display: flex; gap: 8px; align-items: center;",
                    button {
                        r#type: "button",
                        class: "btn-action-sm",
                        onclick: move |_| {
                            on_invite.call((pid, pname_invite.clone()));
                        },
                        "+ Invite"
                    }
                    Link {
                        to: Route::ProjectTasksPage { project_id: pid },
                        style: "color: var(--primary); text-decoration: none; font-weight: var(--font-weight-bold);",
                        "View Board →"
                    }
                }
            }
        }
    }
}
