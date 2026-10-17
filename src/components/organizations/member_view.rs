use dioxus::prelude::*;
use crate::Route;
use crate::types::{Organization, OrganizationMember, OrgProjectAffiliation};
use crate::backend::app_endpoints::remove_organization_member;

#[component]
pub fn OrganizationMemberView(
    organization: Organization,
    members: Vec<OrganizationMember>,
    affiliated_projects: Vec<OrgProjectAffiliation>,
    current_user_id: i64,
) -> Element {
    let org_id = organization.id;
    let mut is_leaving = use_signal(|| false);
    let mut show_leave_confirm = use_signal(|| false);
    let mut leave_error = use_signal(|| Option::<String>::None);
    let nav = use_navigator();

    let my_member = members.iter().find(|m| m.user_id == current_user_id);
    let my_roles = my_member.map(|m| m.roles.clone()).unwrap_or_default();

    let handle_leave = move |_| {
        is_leaving.set(true);
        leave_error.set(None);
        let nav_clone = nav.clone();
        spawn(async move {
            match remove_organization_member(org_id, current_user_id).await {
                Ok(_) => {
                    is_leaving.set(false);
                    nav_clone.push(Route::DashboardPage {});
                }
                Err(e) => {
                    is_leaving.set(false);
                    leave_error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 24px;",
            // Header Card
            div { class: "dashboard-panel",
                div { style: "display: flex; justify-content: space-between; align-items: flex-start; flex-wrap: wrap; gap: 16px;",
                    div {
                        div { style: "display: flex; gap: 10px; align-items: center; margin-bottom: 4px;",
                            h1 { class: "dashboard-title", style: "margin: 0;", "{organization.name}" }
                            span { class: "task-detail-status status-suggested badge-sm", "Member" }
                        }
                        p { style: "margin: 0; color: var(--text-secondary); font-size: var(--font-size-body-sm);",
                            "Slug: " strong { "@{organization.slug}" }
                            " • Governed by " strong { "{organization.governing_user_name}" }
                        }
                    }
                }
            }

            // My Profile & Assigned Roles
            div { class: "dashboard-panel",
                h2 { class: "dashboard-panel-title", "My Assigned Roles" }
                p { style: "margin: 4px 0 12px; color: var(--text-muted); font-size: var(--font-size-body-sm);",
                    "These roles are assigned to you by the organization governor and determine your project grants."
                }

                if my_roles.is_empty() {
                    div { style: "background: var(--bg-surface-raised); border-radius: var(--radius-sm); padding: 16px; border: 1px dashed var(--border-default);",
                        p { style: "margin: 0; color: var(--text-secondary); font-size: var(--font-size-body-sm);",
                            "You are currently a base member of this organization without any specialized functional roles assigned. Contact your organization governor if you require specific role assignments."
                        }
                    }
                } else {
                    div { style: "display: flex; gap: 8px; flex-wrap: wrap;",
                        for r in &my_roles {
                            span {
                                key: "{r}",
                                style: "background-color: var(--primary-bg); color: var(--primary); border: 1px solid var(--primary); border-radius: var(--radius-pill); padding: 4px 12px; font-weight: var(--font-weight-bold); font-size: var(--font-size-body-sm);",
                                "🏷 {r}"
                            }
                        }
                    }
                }
            }

            // Member Directory (Read-only)
            div { class: "dashboard-panel",
                h2 { class: "dashboard-panel-title", "Member Directory ({members.len()})" }
                div { style: "display: flex; flex-direction: column; gap: 12px;",
                    for member in members.iter() {
                        {
                            let is_me = member.user_id == current_user_id;
                            let is_gov = member.user_id == organization.governing_user_id;
                            rsx! {
                                div { key: "{member.user_id}", class: "inbox-item",
                                    div { class: "inbox-item-header",
                                        div { style: "display: flex; gap: 8px; align-items: center;",
                                            span { class: "inbox-item-title", "{member.user_name}" }
                                            if is_me {
                                                span { style: "background-color: var(--bg-surface-hover); color: var(--text-muted); padding: 2px 6px; border-radius: var(--radius-pill); font-size: var(--font-size-meta);", "You" }
                                            }
                                            span { class: "inbox-item-meta", "{member.user_email}" }
                                        }
                                        if is_gov {
                                            span { class: "task-detail-status status-accepted badge-sm", "Governor" }
                                        } else {
                                            span { class: "task-detail-status status-suggested badge-sm", "{member.role}" }
                                        }
                                    }
                                    if !member.roles.is_empty() {
                                        div { style: "display: flex; gap: 6px; flex-wrap: wrap; margin-top: 4px;",
                                            for r in &member.roles {
                                                span {
                                                    key: "{r}",
                                                    style: "background-color: var(--bg-surface-raised); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 2px 8px; font-size: var(--font-size-meta); color: var(--text-secondary);",
                                                    "{r}"
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

            // Affiliated Projects
            div { class: "dashboard-panel",
                h2 { class: "dashboard-panel-title", "Affiliated Projects ({affiliated_projects.len()})" }
                if affiliated_projects.is_empty() {
                    p { class: "empty-description", "No projects are currently affiliated with this organization." }
                } else {
                    div { class: "project-card-list",
                        for p in &affiliated_projects {
                            div { key: "{p.project_id}", class: "project-card",
                                div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                                    Link {
                                        to: Route::ProjectTasksPage { project_id: p.project_id },
                                        class: "project-card-title",
                                        "{p.project_name}"
                                    }
                                    span { class: "task-detail-status status-accepted badge-sm",
                                        "Rel: {p.relationship_type}"
                                    }
                                }
                                div { class: "project-card-meta",
                                    span { "Max Role: {p.max_granted_role}" }
                                    Link {
                                        to: Route::ProjectTasksPage { project_id: p.project_id },
                                        style: "color: var(--primary); text-decoration: none; font-weight: var(--font-weight-bold);",
                                        "Open Board →"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Membership Settings / Leave Organization
            div { class: "dashboard-panel", style: "border: 1px solid var(--border-subtle);",
                h2 { class: "dashboard-panel-title", "Membership Settings" }
                if let Some(err) = &*leave_error.read() {
                    div { class: "task-detail-error", style: "margin-bottom: 12px;", "{err}" }
                }
                div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px;",
                    div {
                        strong { style: "display: block; color: var(--text-primary);", "Leave Organization" }
                        span { style: "font-size: var(--font-size-meta); color: var(--text-muted);",
                            "Remove your membership from this organization and relinquish role grants."
                        }
                    }
                    if *show_leave_confirm.read() {
                        div { style: "display: flex; gap: 8px; align-items: center;",
                            button {
                                r#type: "button",
                                class: "btn-task-cancel",
                                onclick: move |_| show_leave_confirm.set(false),
                                "Cancel"
                            }
                            button {
                                r#type: "button",
                                class: "btn-task-save",
                                style: "background-color: var(--danger); border-color: var(--danger);",
                                disabled: *is_leaving.read(),
                                onclick: handle_leave,
                                if *is_leaving.read() { "Leaving..." } else { "Yes, Leave Organization" }
                            }
                        }
                    } else {
                        button {
                            r#type: "button",
                            class: "btn-action-sm",
                            style: "color: var(--danger); border-color: var(--danger);",
                            onclick: move |_| show_leave_confirm.set(true),
                            "Leave Organization"
                        }
                    }
                }
            }
        }
    }
}
