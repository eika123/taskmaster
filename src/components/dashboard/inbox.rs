use crate::backend::app_endpoints::{
    respond_project_leadership_transfer, respond_to_organization_governance_transfer_request,
    respond_to_organization_invitation, respond_to_project_invitation,
};
use crate::types::UserInbox;
use dioxus::prelude::*;

#[component]
pub fn DashboardInbox(inbox: UserInbox, on_action: EventHandler<()>) -> Element {
    let total_invites = inbox.project_invitations.len()
        + inbox.org_invitations.len()
        + inbox.governance_transfers.len()
        + inbox.leadership_transfers.len();

    rsx! {
        div { class: "dashboard-panel",
            DashboardInboxHeader { total_invites }

            if total_invites == 0 {
                p { class: "empty-description",
                    "Your message box is clear — no pending invites or transfers."
                }
            } else {
                div { class: "inbox-list",
                    for pinv in inbox.project_invitations {
                        ProjectInviteItem {
                            key: "pinv-{pinv.project_id}",
                            pinv,
                            on_action: move |_| on_action.call(()),
                        }
                    }

                    for oinv in inbox.org_invitations {
                        OrgInviteItem {
                            key: "oinv-{oinv.organization_id}",
                            oinv,
                            on_action: move |_| on_action.call(()),
                        }
                    }

                    for gov in inbox.governance_transfers {
                        GovernanceTransferItem {
                            key: "gov-{gov.organization_id}",
                            gov,
                            on_action: move |_| on_action.call(()),
                        }
                    }

                    for lead in inbox.leadership_transfers {
                        LeadershipTransferItem {
                            key: "lead-{lead.project_id}",
                            lead,
                            on_action: move |_| on_action.call(()),
                        }
                    }
                }
            }
        }
    }
}

/// Header for the dashboard inbox displaying the count of pending items.
#[component]
fn DashboardInboxHeader(total_invites: usize) -> Element {
    rsx! {
        h2 { class: "dashboard-panel-title",
            span { "Message Box (Inbox)" }
            if total_invites > 0 {
                span { class: "task-detail-status status-in_progress badge-sm",
                    "{total_invites} Pending"
                }
            }
        }
    }
}

/// Individual project invitation card with accept and decline actions.
#[component]
fn ProjectInviteItem(
    pinv: crate::types::ProjectInvitation,
    on_action: EventHandler<()>,
) -> Element {
    let pid = pinv.project_id;
    rsx! {
        div { class: "inbox-item",
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

/// Individual organization invitation card with accept and decline actions.
#[component]
fn OrgInviteItem(
    oinv: crate::types::OrganizationInvitation,
    on_action: EventHandler<()>,
) -> Element {
    let oid = oinv.organization_id;
    rsx! {
        div { class: "inbox-item",
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

/// Individual organization governance transfer card.
#[component]
fn GovernanceTransferItem(
    gov: crate::types::GovernanceTransferOffer,
    on_action: EventHandler<()>,
) -> Element {
    let oid = gov.organization_id;
    rsx! {
        div {
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
                            let _ = respond_to_organization_governance_transfer_request(oid, false).await;
                            on_action.call(());
                        });
                    },
                    "Decline"
                }
            }
        }
    }
}

/// Individual project leadership transfer card.
#[component]
fn LeadershipTransferItem(
    lead: crate::types::ProjectLeadershipTransferOffer,
    on_action: EventHandler<()>,
) -> Element {
    let pid = lead.project_id;
    rsx! {
        div {
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
