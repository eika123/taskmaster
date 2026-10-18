use super::column::TaskColumn;
use super::modals::InlineCreateTaskModal;
use super::org_modal::ProjectOrganizationsModal;
use crate::backend::app_endpoints::{
    assign_task, delete_project, delete_task, get_my_project_member, get_my_tasks, get_project,
    get_project_assignable_users, get_project_tasks, set_task_status, unassign_task,
};
use crate::components::column_selector::ColumnSelector;
use crate::components::dashboard::ConfirmDeleteModal;
use crate::components::projects::InviteProjectMemberModal;
use crate::types::{has_perm, Permission, Project, Task, TaskStatus, User};
use crate::Route;
use dioxus::prelude::*;

/// Multi-status Kanban task view component (used for My Tasks aggregation and project task board).
#[component]
pub fn TaskView(project_id: Option<i64>) -> Element {
    let mut selected_statuses = use_signal(|| {
        vec![
            TaskStatus::Suggested,
            TaskStatus::Accepted,
            TaskStatus::Assigned,
            TaskStatus::InProgress,
            TaskStatus::Finished,
        ]
    });

    let pid = project_id;
    let mut tasks_resource = use_server_future(move || async move {
        if let Some(p) = pid {
            get_project_tasks(p, None, 100).await
        } else {
            get_my_tasks(None, 100).await
        }
    })?;

    let project_resource = use_server_future(move || async move {
        if let Some(p) = pid {
            get_project(p).await.map(Some)
        } else {
            Ok(None)
        }
    })?;

    let membership_resource = use_server_future(move || async move {
        if let Some(p) = pid {
            get_my_project_member(p).await.ok().flatten()
        } else {
            None
        }
    })?;

    let mut assignable_users_resource =
        use_server_future(move || async move { get_project_assignable_users(pid).await })?;

    let mut task_list = use_signal(|| match &*tasks_resource.read() {
        Some(Ok(list)) => list.clone(),
        _ => Vec::new(),
    });

    let mut all_users = use_signal(|| match &*assignable_users_resource.read() {
        Some(Ok(list)) => list.clone(),
        _ => Vec::new(),
    });

    use_effect(move || {
        if let Some(Ok(list)) = &*tasks_resource.read() {
            task_list.set(list.clone());
        }
    });

    use_effect(move || {
        if let Some(Ok(list)) = &*assignable_users_resource.read() {
            all_users.set(list.clone());
        }
    });

    let mut show_new_task = use_signal(|| false);
    let mut show_invite_modal = use_signal(|| false);
    let mut show_orgs_modal = use_signal(|| false);
    let mut show_delete_modal = use_signal(|| false);
    let mut is_deleting_project = use_signal(|| false);
    let mut delete_proj_err = use_signal(|| Option::<String>::None);
    let auth = use_context::<crate::components::AuthState>();
    let current_user_id = auth.user.read().as_ref().map(|u| u.id);
    let nav = use_navigator();

    let on_dismiss_handler = move |task_id: i64| {
        task_list.with_mut(|tasks| tasks.retain(|t| t.id != task_id));
        spawn(async move {
            let _ = delete_task(task_id).await;
            tasks_resource.restart();
        });
    };

    let on_status_change_handler = move |(task_id, new_status): (u64, TaskStatus)| {
        task_list.with_mut(|tasks| {
            if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id as i64) {
                t.status = new_status;
            }
        });
        spawn(async move {
            let _ = set_task_status(task_id, new_status).await;
            tasks_resource.restart();
        });
    };

    let on_assign_handler = move |(task_id, user_id): (i64, i64)| {
        spawn(async move {
            if assign_task(task_id, user_id).await.is_ok() {
                tasks_resource.restart();
            }
        });
    };

    let on_unassign_handler = move |(task_id, user_id): (i64, i64)| {
        spawn(async move {
            if unassign_task(task_id, user_id).await.is_ok() {
                tasks_resource.restart();
            }
        });
    };

    let project_info = match &*project_resource.read() {
        Some(Ok(Some(proj))) => Some(proj.clone()),
        _ => None,
    };

    let my_membership = match &*membership_resource.read() {
        Some(Some(m)) => Some(m.clone()),
        _ => None,
    };

    let can_mutate = if pid.is_some() {
        my_membership
            .as_ref()
            .map(|m| has_perm(m.permissions_mask, Permission::EXECUTE))
            .unwrap_or(false)
    } else {
        true
    };

    if let Some(Err(err)) = &*project_resource.read() {
        return rsx! { TaskViewAccessDenied { err: err.to_string() } };
    }
    if let Some(Err(err)) = &*tasks_resource.read() {
        return rsx! { TaskViewAccessDenied { err: err.to_string() } };
    }

    let is_leader = pid.is_some() && project_info.as_ref().map(|p| p.leader_id) == current_user_id;

    rsx! {
        div { class: "task-view",
            TaskViewTopBar {
                project_info: project_info.clone(),
                has_pid: pid.is_some(),
                can_mutate,
                is_leader,
                pid,
                on_toggle_invite: move |_| {
                    let curr = *show_invite_modal.read();
                    show_invite_modal.set(!curr);
                },
                on_toggle_orgs: move |_| {
                    let curr = *show_orgs_modal.read();
                    show_orgs_modal.set(!curr);
                },
                on_open_delete: move |_| show_delete_modal.set(true),
                on_toggle_new_task: move |_| {
                    let curr = *show_new_task.read();
                    show_new_task.set(!curr);
                },
            }

            TaskViewModals {
                pid,
                project_info: project_info.clone(),
                show_delete_modal: *show_delete_modal.read(),
                is_deleting_project: *is_deleting_project.read(),
                delete_proj_err: (*delete_proj_err.read()).clone(),
                show_new_task: *show_new_task.read(),
                show_invite_modal: *show_invite_modal.read(),
                show_orgs_modal: *show_orgs_modal.read(),
                on_close_delete: move |_| {
                    show_delete_modal.set(false);
                    delete_proj_err.set(None);
                },
                on_confirm_delete: move |confirmation: String| {
                    if let Some(target_pid) = pid {
                        is_deleting_project.set(true);
                        delete_proj_err.set(None);
                        let nav = nav.clone();
                        spawn(async move {
                            match delete_project(target_pid, confirmation).await {
                                Ok(_) => {
                                    is_deleting_project.set(false);
                                    show_delete_modal.set(false);
                                    nav.push(Route::DashboardPage {});
                                }
                                Err(e) => {
                                    is_deleting_project.set(false);
                                    delete_proj_err.set(Some(e.to_string()));
                                }
                            }
                        });
                    }
                },
                on_close_new_task: move |_| show_new_task.set(false),
                on_task_created: move |_| {
                    show_new_task.set(false);
                    tasks_resource.restart();
                },
                on_close_invite: move |_| show_invite_modal.set(false),
                on_invited: move |_| {
                    show_invite_modal.set(false);
                    assignable_users_resource.restart();
                },
                on_close_orgs: move |_| show_orgs_modal.set(false),
            }

            ColumnSelector {
                selected_statuses: selected_statuses(),
                on_toggle: move |status: TaskStatus| {
                    let mut list = selected_statuses();
                    if let Some(pos) = list.iter().position(|&s| s == status) {
                        list.remove(pos);
                    } else {
                        list.push(status);
                    }
                    selected_statuses.set(list);
                },
                on_select_all: move |_| {
                    selected_statuses.set(vec![
                        TaskStatus::Suggested,
                        TaskStatus::Accepted,
                        TaskStatus::Assigned,
                        TaskStatus::InProgress,
                        TaskStatus::Finished,
                    ]);
                },
                on_clear: move |_| selected_statuses.set(Vec::new()),
            }

            TaskColumnsContainer {
                active_statuses: selected_statuses(),
                tasks: task_list(),
                all_users: all_users(),
                can_mutate,
                on_dismiss: on_dismiss_handler,
                on_status_change: on_status_change_handler,
                on_assign: on_assign_handler,
                on_unassign: on_unassign_handler,
            }
        }
    }
}

/// Access denied error display card.
#[component]
fn TaskViewAccessDenied(err: String) -> Element {
    rsx! {
        div { class: "dashboard-panel",
            style: "border: 1px solid var(--danger-bg); margin-top: 16px; background-color: var(--bg-surface); padding: 24px; border-radius: var(--radius-md);",
            h2 { style: "color: var(--danger); margin-bottom: 8px; font-size: var(--font-size-h3);",
                "Access Denied"
            }
            p { style: "color: var(--text-muted); font-size: var(--font-size-body);",
                "{err}"
            }
            div { style: "margin-top: 16px;",
                Link {
                    to: Route::TasksDashboardPage {},
                    class: "btn-back-link",
                    "← Return to My Tasks"
                }
            }
        }
    }
}

/// Top bar with project details and action buttons for TaskView.
#[component]
fn TaskViewTopBar(
    project_info: Option<Project>,
    has_pid: bool,
    can_mutate: bool,
    is_leader: bool,
    pid: Option<i64>,
    on_toggle_invite: EventHandler<()>,
    on_toggle_orgs: EventHandler<()>,
    on_open_delete: EventHandler<()>,
    on_toggle_new_task: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "task-view-topbar",
            div {
                if let Some(proj) = &project_info {
                    h1 { class: "task-view-title", "{proj.name}" }
                    div { style: "display: flex; gap: 8px; align-items: center; margin-top: 4px;",
                        span { style: "color: var(--text-muted); font-size: var(--font-size-meta);",
                            "Led by "
                            strong { "{proj.leader_name}" }
                        }
                        if let Some(org_name) = &proj.owner_org_name {
                            span { class: "task-detail-status status-accepted badge-sm", "{org_name}" }
                        } else {
                            span { class: "task-detail-status status-suggested badge-sm", "Standalone" }
                        }
                    }
                } else {
                    h1 { class: "task-view-title", "My Tasks" }
                    p { style: "margin: 4px 0 0; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                        "Tasks assigned to or created by you across all projects"
                    }
                }
            }

            div { style: "display: flex; gap: 10px; align-items: center;",
                if has_pid && can_mutate {
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| on_toggle_invite.call(()),
                        "👥 Invite Member"
                    }
                }
                if is_leader && pid.is_some() {
                    Link {
                        to: Route::ProjectManagePage { project_id: pid.unwrap() },
                        class: "btn-task-cancel",
                        style: "text-decoration: none;",
                        "⚙ Manage Project"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        onclick: move |_| on_toggle_orgs.call(()),
                        "🏢 Organizations"
                    }
                    button {
                        r#type: "button",
                        class: "btn-task-cancel",
                        style: "color: var(--danger); border-color: var(--danger-bg);",
                        onclick: move |_| on_open_delete.call(()),
                        "🗑 Delete Project"
                    }
                }
                if can_mutate {
                    button {
                        r#type: "button",
                        class: "btn-create-task",
                        onclick: move |_| on_toggle_new_task.call(()),
                        span { class: "btn-create-icon", "+" }
                        " New Task"
                    }
                }
            }
        }
    }
}

/// Modals wrapper component for task and project operations.
#[component]
fn TaskViewModals(
    pid: Option<i64>,
    project_info: Option<Project>,
    show_delete_modal: bool,
    is_deleting_project: bool,
    delete_proj_err: Option<String>,
    show_new_task: bool,
    show_invite_modal: bool,
    show_orgs_modal: bool,
    on_close_delete: EventHandler<()>,
    on_confirm_delete: EventHandler<String>,
    on_close_new_task: EventHandler<()>,
    on_task_created: EventHandler<()>,
    on_close_invite: EventHandler<()>,
    on_invited: EventHandler<()>,
    on_close_orgs: EventHandler<()>,
) -> Element {
    rsx! {
        if show_delete_modal {
            if let Some(pinfo) = &project_info {
                ConfirmDeleteModal {
                    entity_type: "project".to_string(),
                    entity_name: pinfo.name.clone(),
                    on_close: move |_| on_close_delete.call(()),
                    on_confirm: move |phrase| on_confirm_delete.call(phrase),
                    is_deleting: is_deleting_project,
                    error: delete_proj_err,
                }
            }
        }

        if show_new_task {
            InlineCreateTaskModal {
                project_id: pid,
                on_close: move |_| on_close_new_task.call(()),
                on_created: move |_| on_task_created.call(()),
            }
        }

        if show_invite_modal && pid.is_some() {
            InviteProjectMemberModal {
                project_id: pid.unwrap(),
                project_name: project_info.as_ref().map(|p| p.name.clone()),
                on_close: move |_| on_close_invite.call(()),
                on_invited: move |_| on_invited.call(()),
            }
        }

        if show_orgs_modal {
            if let (Some(target_pid), Some(pinfo)) = (pid, &project_info) {
                ProjectOrganizationsModal {
                    project_id: target_pid,
                    project_name: pinfo.name.clone(),
                    on_close: move |_| on_close_orgs.call(()),
                }
            }
        }
    }
}

/// Horizontal scrollable columns container for TaskView.
#[component]
fn TaskColumnsContainer(
    active_statuses: Vec<TaskStatus>,
    tasks: Vec<Task>,
    all_users: Vec<User>,
    can_mutate: bool,
    on_dismiss: EventHandler<i64>,
    on_status_change: EventHandler<(u64, TaskStatus)>,
    on_assign: EventHandler<(i64, i64)>,
    on_unassign: EventHandler<(i64, i64)>,
) -> Element {
    rsx! {
        if active_statuses.is_empty() {
            div { class: "no-columns-notice",
                p { "No columns selected. Click one or more status pills above to view tasks." }
            }
        } else {
            div { class: "task-board-scroll-container",
                div { class: "task-columns-row",
                    for status in active_statuses {
                        TaskColumn {
                            key: "{status.as_str()}",
                            status,
                            tasks: tasks.clone(),
                            all_users: all_users.clone(),
                            can_mutate,
                            on_dismiss,
                            on_status_change,
                            on_assign,
                            on_unassign,
                        }
                    }
                }
            }
        }
    }
}
