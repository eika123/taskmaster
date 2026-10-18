use super::{
    CreateOrgModal, CreateProjectModal, DashboardInbox, DashboardOrganizations, DashboardProjects,
    ManageOrgModal,
};
use crate::backend::app_endpoints::get_user_dashboard_data;
use crate::components::projects::InviteProjectMemberModal;
use crate::types::Organization;
use dioxus::prelude::*;

#[component]
pub fn DashboardPage() -> Element {
    let mut dashboard_resource =
        use_server_future(move || async move { get_user_dashboard_data().await })?;

    let mut show_create_project = use_signal(|| false);
    let mut show_create_org = use_signal(|| false);
    let mut active_invite_proj = use_signal(|| Option::<(i64, String)>::None);
    let mut active_manage_org = use_signal(|| None::<Organization>);

    let data_read = dashboard_resource.read();
    let dashboard_data = match &*data_read {
        Some(Ok(d)) => Some(d.clone()),
        _ => None,
    };

    rsx! {
        div { class: "dashboard-container",
            DashboardTopHeader {
                on_toggle_new_project: move |_| {
                    let curr = *show_create_project.read();
                    show_create_project.set(!curr);
                },
                on_toggle_new_org: move |_| {
                    let curr = *show_create_org.read();
                    show_create_org.set(!curr);
                },
            }

            DashboardModalsHost {
                show_create_project: *show_create_project.read(),
                show_create_org: *show_create_org.read(),
                organizations: dashboard_data.as_ref().map(|d| d.organizations.clone()).unwrap_or_default(),
                active_invite_proj: active_invite_proj.read().clone(),
                active_manage_org: active_manage_org.read().clone(),
                on_close_create_project: move |_| show_create_project.set(false),
                on_project_created: move |_| {
                    show_create_project.set(false);
                    dashboard_resource.restart();
                },
                on_close_create_org: move |_| show_create_org.set(false),
                on_org_created: move |_| {
                    show_create_org.set(false);
                    dashboard_resource.restart();
                },
                on_close_invite: move |_| active_invite_proj.set(None),
                on_invited: move |_| {
                    active_invite_proj.set(None);
                    dashboard_resource.restart();
                },
                on_close_manage_org: move |_| active_manage_org.set(None),
                on_org_deleted: move |_| {
                    active_manage_org.set(None);
                    dashboard_resource.restart();
                },
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

/// Top header bar for the dashboard with primary creation actions.
#[component]
fn DashboardTopHeader(
    on_toggle_new_project: EventHandler<()>,
    on_toggle_new_org: EventHandler<()>,
) -> Element {
    rsx! {
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
                    onclick: move |_| on_toggle_new_project.call(()),
                    "+ New Project"
                }
                button {
                    r#type: "button",
                    class: "btn-task-cancel",
                    onclick: move |_| on_toggle_new_org.call(()),
                    "+ New Organization"
                }
            }
        }
    }
}

/// Host component rendering any open dashboard modal dialog.
#[component]
fn DashboardModalsHost(
    show_create_project: bool,
    show_create_org: bool,
    organizations: Vec<Organization>,
    active_invite_proj: Option<(i64, String)>,
    active_manage_org: Option<Organization>,
    on_close_create_project: EventHandler<()>,
    on_project_created: EventHandler<()>,
    on_close_create_org: EventHandler<()>,
    on_org_created: EventHandler<()>,
    on_close_invite: EventHandler<()>,
    on_invited: EventHandler<()>,
    on_close_manage_org: EventHandler<()>,
    on_org_deleted: EventHandler<()>,
) -> Element {
    rsx! {
        if show_create_project {
            CreateProjectModal {
                organizations,
                on_close: move |_| on_close_create_project.call(()),
                on_created: move |_| on_project_created.call(()),
            }
        }

        if show_create_org {
            CreateOrgModal {
                on_close: move |_| on_close_create_org.call(()),
                on_created: move |_| on_org_created.call(()),
            }
        }

        if let Some((pid, pname)) = active_invite_proj {
            InviteProjectMemberModal {
                project_id: pid,
                project_name: Some(pname),
                on_close: move |_| on_close_invite.call(()),
                on_invited: move |_| on_invited.call(()),
            }
        }

        if let Some(org) = active_manage_org {
            ManageOrgModal {
                organization: org,
                on_close: move |_| on_close_manage_org.call(()),
                on_deleted: move |_| on_org_deleted.call(()),
            }
        }
    }
}
