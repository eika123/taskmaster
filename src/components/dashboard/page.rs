use dioxus::prelude::*;
use crate::backend::app_endpoints::get_user_dashboard_data;
use crate::components::projects::InviteProjectMemberModal;
use super::{
    CreateOrgModal, CreateProjectModal, DashboardInbox, DashboardOrganizations, DashboardProjects,
    ManageOrgModal,
};

#[component]
pub fn DashboardPage() -> Element {
    let mut dashboard_resource = use_server_future(move || async move {
        get_user_dashboard_data().await
    })?;

    let mut show_create_project = use_signal(|| false);
    let mut show_create_org = use_signal(|| false);
    let mut active_invite_proj = use_signal(|| Option::<(i64, String)>::None);
    let mut active_manage_org = use_signal(|| Option::<crate::types::Organization>::None);

    let data_read = dashboard_resource.read();
    let dashboard_data = match &*data_read {
        Some(Ok(d)) => Some(d.clone()),
        _ => None,
    };

    rsx! {
        div { class: "dashboard-container",
            div { class: "dashboard-header",
                div {
                    h1 { class: "dashboard-title", "User Dashboard" }
                    p { style: "margin: 4px 0 0; color: var(--text-secondary);",
                        "Manage your active projects, organizations, and pending invitations."
                    }
                }
                div { style: "display: flex; gap: 10px;",
                    button {
                        r#type: "button",
                        class: "btn-task-edit",
                        onclick: move |_| {
                            let curr = *show_create_project.read();
                            show_create_project.set(!curr);
                        },
                        "+ New Project"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| {
                            let curr = *show_create_org.read();
                            show_create_org.set(!curr);
                        },
                        "+ New Organization"
                    }
                }
            }

            if *show_create_project.read() {
                CreateProjectModal {
                    organizations: dashboard_data.as_ref().map(|d| d.organizations.clone()).unwrap_or_default(),
                    on_close: move |_| show_create_project.set(false),
                    on_created: move |_| {
                        show_create_project.set(false);
                        dashboard_resource.restart();
                    },
                }
            }

            if *show_create_org.read() {
                CreateOrgModal {
                    on_close: move |_| show_create_org.set(false),
                    on_created: move |_| {
                        show_create_org.set(false);
                        dashboard_resource.restart();
                    },
                }
            }

            if let Some((pid, pname)) = active_invite_proj.read().clone() {
                InviteProjectMemberModal {
                    project_id: pid,
                    project_name: Some(pname),
                    on_close: move |_| active_invite_proj.set(None),
                    on_invited: move |_| {
                        active_invite_proj.set(None);
                        dashboard_resource.restart();
                    },
                }
            }

            if let Some(org) = active_manage_org.read().clone() {
                ManageOrgModal {
                    organization: org,
                    on_close: move |_| active_manage_org.set(None),
                    on_deleted: move |_| {
                        active_manage_org.set(None);
                        dashboard_resource.restart();
                    },
                }
            }

            if let Some(data) = dashboard_data {
                div { class: "dashboard-grid",
                    div { style: "display: flex; flex-direction: column; gap: 28px;",
                        DashboardProjects {
                            projects: data.projects,
                            on_invite: move |p| active_invite_proj.set(Some(p)),
                        }
                        DashboardOrganizations {
                            organizations: data.organizations,
                            on_manage: move |org| active_manage_org.set(Some(org)),
                        }
                    }
                    DashboardInbox {
                        inbox: data.inbox,
                        on_action: move |_| dashboard_resource.restart(),
                    }
                }
            }
        }
    }
}

