use dioxus::prelude::*;
use crate::backend::app_endpoints::{
    respond_to_organization_governance_transfer_request, respond_project_leadership_transfer,
    respond_to_organization_invitation, respond_to_project_invitation,
};
use crate::types::UserInbox;

#[component]
pub fn DashboardInbox(
    inbox: UserInbox,
    on_action: EventHandler<()>,
) -> Element {
    let total_invites = inbox.project_invitations.len()
        + inbox.org_invitations.len()
        + inbox.governance_transfers.len()
        + inbox.leadership_transfers.len();

    rsx! {
        div { class: "dashboard-panel",
            h2 { class: "dashboard-panel-title",
                span { "Message Box (Inbox)" }
                if total_invites > 0 {
                    span { class: "task-detail-status status-in_progress badge-sm",
                        "{total_invites} Pending"
                    }
                }
            }

            if total_invites == 0 {
                p { class: "empty-description",
                    "Your message box is clear — no pending invites or transfers."
                }
            } else {
                div { class: "inbox-list",
                    // 1. Project Invitations
                    for pinv in inbox.project_invitations {
                        {
                            let pid = pinv.project_id;
                            rsx! {
                                div { key: "pinv-{pid}", class: "inbox-item",
                                    div { class: "inbox-item-header",
                                        span { class: "inbox-item-title", "Project Invitation: {pinv.project_name}" }
                                        span { class: "inbox-item-meta", "Role: {pinv.role}" }
                                    }
                                    p { class: "inbox-item-msg",
                                        "Invited by {pinv.invited_by_name}."
                                        if !pinv.message.is_empty() {
                                            span { style: "display: block; margin-top: 4px; font-style: italic;", "\"{pinv.message}\"" }
                                        }
                                    }
                                    div { class: "inbox-item-actions",
                                        button {
                                            r#type: "button",
                                            class: "btn-inbox-accept",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    let _ = respond_to_project_invitation(pid, true).await;
                                                    on_action.call(());
                                                });
                                            },
                                            "Accept"
                                        }
                                        button {
                                            r#type: "button",
                                            class: "btn-inbox-decline",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    let _ = respond_to_project_invitation(pid, false).await;
                                                    on_action.call(());
                                                });
                                            },
                                            "Decline"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 2. Organization Invitations
                    for oinv in inbox.org_invitations {
                        {
                            let oid = oinv.organization_id;
                            rsx! {
                                div { key: "oinv-{oid}", class: "inbox-item",
                                    div { class: "inbox-item-header",
                                        span { class: "inbox-item-title", "Org Invitation: {oinv.organization_name}" }
                                        span { class: "inbox-item-meta", "Role: {oinv.role}" }
                                    }
                                    p { class: "inbox-item-msg",
                                        "Invited by {oinv.invited_by_name}."
                                        if !oinv.message.is_empty() {
                                            span { style: "display: block; margin-top: 4px; font-style: italic;", "\"{oinv.message}\"" }
                                        }
                                    }
                                    div { class: "inbox-item-actions",
                                        button {
                                            r#type: "button",
                                            class: "btn-inbox-accept",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    let _ = respond_to_organization_invitation(oid, true).await;
                                                    on_action.call(());
                                                });
                                            },
                                            "Accept"
                                        }
                                        button {
                                            r#type: "button",
                                            class: "btn-inbox-decline",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    let _ = respond_to_organization_invitation(oid, false).await;
                                                    on_action.call(());
                                                });
                                            },
                                            "Decline"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 3. Governance Transfers
                    for gov in inbox.governance_transfers {
                        {
                            let oid = gov.organization_id;
                            rsx! {
                                div {
                                    key: "gov-{oid}",
                                    class: "inbox-item",
                                    style: "border-left: 3px solid var(--primary);",
                                    div { class: "inbox-item-header",
                                        span { class: "inbox-item-title", "Governance Transfer Offer" }
                                    }
                                    p { class: "inbox-item-msg",
                                        "{gov.current_governor_name} has requested that you take over governance of "
                                        strong { "{gov.organization_name}" }
                                        "."
                                    }
                                    div { class: "inbox-item-actions",
                                        button {
                                            r#type: "button",
                                            class: "btn-inbox-accept",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    let _ = respond_to_organization_governance_transfer_request(oid, true).await;
                                                    on_action.call(());
                                                });
                                            },
                                            "Accept Governance"
                                        }
                                        button {
                                            r#type: "button",
                                            class: "btn-inbox-decline",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    let _ = respond_to_organization_governance_transfer_request(oid, false)
                                                        .await;
                                                    on_action.call(());
                                                });
                                            },
                                            "Decline"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 4. Leadership Transfers
                    for lead in inbox.leadership_transfers {
                        {
                            let pid = lead.project_id;
                            rsx! {
                                div {
                                    key: "lead-{pid}",
                                    class: "inbox-item",
                                    style: "border-left: 3px solid var(--status-in_progress);",
                                    div { class: "inbox-item-header",
                                        span { class: "inbox-item-title", "Project Leadership Transfer Offer" }
                                    }
                                    p { class: "inbox-item-msg",
                                        "{lead.current_leader_name} has requested that you become the new leader of "
                                        strong { "{lead.project_name}" }
                                        "."
                                    }
                                    div { class: "inbox-item-actions",
                                        button {
                                            r#type: "button",
                                            class: "btn-inbox-accept",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    let _ = respond_project_leadership_transfer(pid, true).await;
                                                    on_action.call(());
                                                });
                                            },
                                            "Accept Leadership"
                                        }
                                        button {
                                            r#type: "button",
                                            class: "btn-inbox-decline",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    let _ = respond_project_leadership_transfer(pid, false).await;
                                                    on_action.call(());
                                                });
                                            },
                                            "Decline"
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

