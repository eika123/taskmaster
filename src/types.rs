#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(rename_all = "snake_case"))]
pub enum TaskStatus {
    Suggested,
    Accepted,
    Assigned,
    InProgress,
    Finished,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Suggested => "suggested",
            Self::Accepted => "accepted",
            Self::Assigned => "assigned",
            Self::InProgress => "in_progress",
            Self::Finished => "finished",
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "suggested" => Ok(Self::Suggested),
            "accepted" => Ok(Self::Accepted),
            "assigned" => Ok(Self::Assigned),
            "in_progress" => Ok(Self::InProgress),
            "finished" => Ok(Self::Finished),
            other => Err(format!("Unknown task status: '{other}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
}

impl Default for User {
    fn default() -> Self {
        Self {
            id: 0,
            name: "Anonymous".to_string(),
            email: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub title: String,
    pub author: Option<i64>,
    pub author_name: String,
    pub description: String,
    pub status: TaskStatus,
    pub assignees: Vec<User>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct TaskMessage {
    pub task_id: i64,
    pub msg_id: i64,
    pub author_id: Option<i64>,
    pub author_name: String,
    pub created_at: i64,
    pub msg: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Organization {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub governing_user_id: i64,
    pub governing_user_name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrganizationMember {
    pub organization_id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub user_email: String,
    pub role: String,
    pub joined_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub owner_org_id: Option<i64>,
    pub owner_org_name: Option<String>,
    pub leader_id: i64,
    pub leader_name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectMember {
    pub project_id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub user_email: String,
    pub role: String,
    pub granted_by_org_role: bool,
    pub joined_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectOrgRoleAccess {
    pub project_id: i64,
    pub organization_id: i64,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectInvitation {
    pub project_id: i64,
    pub project_name: String,
    pub invited_user_id: i64,
    pub invited_by: Option<i64>,
    pub invited_by_name: String,
    pub role: String,
    pub message: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OrganizationInvitation {
    pub organization_id: i64,
    pub organization_name: String,
    pub invited_user_id: i64,
    pub invited_by: Option<i64>,
    pub invited_by_name: String,
    pub role: String,
    pub message: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GovernanceTransferOffer {
    pub organization_id: i64,
    pub organization_name: String,
    pub requested_new_governor: i64,
    pub current_governor_name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectLeadershipTransferOffer {
    pub project_id: i64,
    pub project_name: String,
    pub requested_new_leader: i64,
    pub current_leader_name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UserInbox {
    pub project_invitations: Vec<ProjectInvitation>,
    pub org_invitations: Vec<OrganizationInvitation>,
    pub governance_transfers: Vec<GovernanceTransferOffer>,
    pub leadership_transfers: Vec<ProjectLeadershipTransferOffer>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DashboardData {
    pub inbox: UserInbox,
    pub projects: Vec<Project>,
    pub organizations: Vec<Organization>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

// Re-export endpoints, database, and auth middleware for convenience
#[cfg(feature = "server")]
pub use crate::backend::database::get_db;
#[cfg(feature = "server")]
pub use crate::backend::middleware::auth::*;

use validator::ValidateEmail;

/// Validates that an email is RFC/HTML5 compliant and has a valid domain with a TLD (e.g. `user@example.com`).
/// Rejects incomplete domains like `user@example` or `user@example.` to prevent UI flashing while typing.
pub fn is_valid_email(email: &str) -> bool {
    let clean = email.trim();
    if !clean.validate_email() {
        return false;
    }
    if let Some(domain) = clean.rsplit('@').next() {
        if let Some(tld) = domain.rsplit('.').next() {
            return domain.contains('.')
                && tld.len() >= 2
                && !domain.starts_with('.')
                && !domain.ends_with('.');
        }
    }
    false
}

