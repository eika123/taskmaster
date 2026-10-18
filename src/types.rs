/// Canonical task execution status within the legacy 5-state Kanban lifecycle.
///
/// During Milestone M1 (Expand Phase), `TaskStatus` is dual-written alongside
/// `project_workflow_status.id` to guarantee zero-downtime backward compatibility
/// across existing Kanban boards and client RPC contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Newly proposed task awaiting review or triage (`allows_assignment = false`).
    Suggested,
    /// Accepted task ready to be scheduled or assigned.
    Accepted,
    /// Task assigned to one or more contributors.
    Assigned,
    /// Active work in progress (`requires_assignee = true`).
    InProgress,
    /// Terminal state denoting successful completion.
    Finished,
}

// ============================================================================
// SQLx Database Mapping: TaskStatus <-> PostgreSQL TEXT
// ============================================================================

#[cfg(feature = "server")]
/// Declares SQL type metadata for [`TaskStatus`], mapping it to PostgreSQL `TEXT`/`VARCHAR`.
impl sqlx::Type<sqlx::Postgres> for TaskStatus {
    /// Returns the database type metadata for PostgreSQL, matching standard `TEXT`.
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    /// Verifies whether a PostgreSQL column type is compatible with this enum's text representation.
    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

#[cfg(feature = "server")]
/// Encodes [`TaskStatus`] into the PostgreSQL wire format by serializing its snake_case string representation.
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for TaskStatus {
    /// Encodes the enum variant as a string slice into the PostgreSQL argument buffer.
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <&str as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&self.as_str(), buf)
    }
}

#[cfg(feature = "server")]
/// Decodes a [`TaskStatus`] from a PostgreSQL text column or query result.
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for TaskStatus {
    /// Parses a database string value into the matching [`TaskStatus`] variant, returning an error for unknown strings.
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let s = <&str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        s.parse::<Self>().map_err(Into::into)
    }
}

impl TaskStatus {
    /// Returns the canonical snake_case string literal matching the PostgreSQL `task.status` column check constraint.
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
    /// Writes the canonical snake_case string to the formatter.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    /// Attempts to parse standard snake_case status names (`suggested`, `accepted`, `assigned`, `in_progress`, `finished`).
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

use chrono::{DateTime, Utc};

/// High-level workflow category grouping custom project statuses for cross-project dashboards.
///
/// While every project may define custom status names (e.g. "Consumables Farming", "Code Review"),
/// the global dashboard and rollup views aggregate tasks by their semantic [`StatusCategory`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusCategory {
    /// Backlog, triage, or unstarted tasks.
    Todo,
    /// Active work or implementation in progress.
    InProgress,
    /// Peer review, QA verification, or testing.
    Review,
    /// Successfully completed or delivered work.
    Done,
    /// Archived, cancelled, or deprecated tasks.
    Archived,
}

// ============================================================================
// SQLx Database Mapping: StatusCategory <-> PostgreSQL TEXT
// ============================================================================

#[cfg(feature = "server")]
/// Declares SQL type metadata for [`StatusCategory`], mapping it to PostgreSQL `TEXT`/`VARCHAR`.
impl sqlx::Type<sqlx::Postgres> for StatusCategory {
    /// Returns PostgreSQL type metadata matching standard `TEXT`.
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    /// Verifies compatibility with PostgreSQL string column types.
    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

#[cfg(feature = "server")]
/// Encodes [`StatusCategory`] into the PostgreSQL wire format as its snake_case string.
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for StatusCategory {
    /// Writes the category string slice into the PostgreSQL argument buffer.
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <&str as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&self.as_str(), buf)
    }
}

#[cfg(feature = "server")]
/// Decodes a [`StatusCategory`] from a PostgreSQL text column or query result.
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for StatusCategory {
    /// Decodes a raw database text value into the corresponding [`StatusCategory`] variant.
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let s = <&str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        s.parse::<Self>().map_err(Into::into)
    }
}

impl StatusCategory {
    /// Returns the canonical snake_case string literal matching the PostgreSQL `project_workflow_status.category` check constraint.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Review => "review",
            Self::Done => "done",
            Self::Archived => "archived",
        }
    }
}

/// Formats the [`StatusCategory`] as its canonical string for display and logging.
impl std::fmt::Display for StatusCategory {
    /// Writes the canonical snake_case string to the formatter.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parses a string into a [`StatusCategory`] variant.
impl std::str::FromStr for StatusCategory {
    type Err = String;

    /// Parses string inputs such as `todo`, `in_progress`, `review`, `done`, `archived`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "todo" => Ok(Self::Todo),
            "in_progress" | "inprogress" => Ok(Self::InProgress),
            "review" => Ok(Self::Review),
            "done" => Ok(Self::Done),
            "archived" => Ok(Self::Archived),
            other => Err(format!("Unknown status category: '{other}'")),
        }
    }
}

/// Visual badge and column highlight styling for workflow statuses on Kanban boards.
///
/// Maps directly to semantic CSS utility styles and column headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeVariant {
    Default,
    Info,
    Success,
    Warning,
    Danger,
    Purple,
    Teal,
}

// ============================================================================
// SQLx Database Mapping: BadgeVariant <-> PostgreSQL TEXT
// ============================================================================

#[cfg(feature = "server")]
/// Declares SQL type metadata for [`BadgeVariant`], mapping it to PostgreSQL `TEXT`/`VARCHAR`.
impl sqlx::Type<sqlx::Postgres> for BadgeVariant {
    /// Returns PostgreSQL type metadata matching standard `TEXT`.
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    /// Verifies compatibility with PostgreSQL string column types.
    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

#[cfg(feature = "server")]
/// Encodes [`BadgeVariant`] into the PostgreSQL wire format as its snake_case string.
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for BadgeVariant {
    /// Writes the badge variant string slice into the PostgreSQL argument buffer.
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <&str as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&self.as_str(), buf)
    }
}

#[cfg(feature = "server")]
/// Decodes a [`BadgeVariant`] from a PostgreSQL text column or query result.
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for BadgeVariant {
    /// Decodes a raw database text value into the corresponding [`BadgeVariant`] variant.
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let s = <&str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        s.parse::<Self>().map_err(Into::into)
    }
}

impl BadgeVariant {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Danger => "danger",
            Self::Purple => "purple",
            Self::Teal => "teal",
        }
    }
}

impl std::fmt::Display for BadgeVariant {
    /// Writes the canonical snake_case string to the formatter.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for BadgeVariant {
    type Err = String;

    /// Parses string inputs such as `default`, `info`, `success`, `warning`, `danger`, `purple`, `teal`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(Self::Default),
            "info" => Ok(Self::Info),
            "success" => Ok(Self::Success),
            "warning" => Ok(Self::Warning),
            "danger" => Ok(Self::Danger),
            "purple" => Ok(Self::Purple),
            "teal" => Ok(Self::Teal),
            other => Err(format!("Unknown badge variant: '{other}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct WorkflowStatusDto {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub category: StatusCategory,
    pub badge_variant: BadgeVariant,
    pub position: i64,
    pub wip_limit: Option<i32>,
    pub is_initial: bool,
    pub allows_assignment: bool,
    pub requires_assignee: bool,
    #[serde(default)]
    pub task_count: i64,
    #[serde(default)]
    pub wip_exceeded: bool,
    pub created_at: DateTime<Utc>,
}

impl WorkflowStatusDto {
    pub fn with_task_count(mut self, count: i64) -> Self {
        self.task_count = count;
        self.wip_exceeded = self.wip_limit.is_some_and(|limit| count > limit as i64);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectWorkflowTransitionDto {
    pub project_id: i64,
    pub from_status_id: i64,
    pub to_status_id: i64,
    pub required_permission: i64,
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
    #[serde(default)]
    pub workflow_status_id: Option<i64>,
    #[serde(default)]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub due_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub estimated_hours: Option<i32>,
    #[serde(default)]
    pub progress_percent: i32,
    pub assignees: Vec<User>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct Permission(pub u64);

impl Permission {
    pub const READ: Self = Self(1);
    pub const SUGGEST: Self = Self(2);
    pub const EXECUTE: Self = Self(4);
    pub const ASSIGN: Self = Self(8);
    pub const MANAGE_MEMBERS: Self = Self(16);
    pub const ADMIN: Self = Self(32);
    pub const MANAGE_WORKFLOW: Self = Self(64);
    pub const MODERATE_DISCUSSIONS: Self = Self(128);
    pub const POST_ANNOUNCEMENTS: Self = Self(256);
    pub const MESSAGE_MEMBERS: Self = Self(512);
    pub const IGNORABLE_BY_MEMBERS: Self = Self(1024);

    // Pre-composed role defaults:
    pub const VIEWER_DEFAULT: Self = Self(Self::READ.0);
    pub const MEMBER_DEFAULT: Self = Self(
        Self::READ.0 | Self::SUGGEST.0 | Self::EXECUTE.0 | Self::ASSIGN.0 | Self::MESSAGE_MEMBERS.0 | Self::IGNORABLE_BY_MEMBERS.0
    );
    pub const LEGACY_MEMBER: Self = Self(15);
    pub const LEGACY_ADMIN: Self = Self(63);
    pub const ADMIN_DEFAULT: Self = Self(
        Self::READ.0 | Self::SUGGEST.0 | Self::EXECUTE.0 | Self::ASSIGN.0 | Self::MANAGE_MEMBERS.0
        | Self::ADMIN.0 | Self::MANAGE_WORKFLOW.0 | Self::MODERATE_DISCUSSIONS.0
        | Self::POST_ANNOUNCEMENTS.0 | Self::MESSAGE_MEMBERS.0
    );

    #[inline]
    pub const fn bits(&self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn as_i64(&self) -> i64 {
        self.0 as i64
    }

    #[inline]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn intersects(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[inline]
    pub const fn has(&self, other: Self) -> bool {
        self.intersects(other)
    }
}

/// Combines two permission masks using bitwise OR (`|`).
impl std::ops::BitOr for Permission {
    type Output = Self;
    /// Returns the union of both permission sets.
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for Permission {
    type Output = Self;
    /// Returns the common permissions present in both sets.
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for Permission {
    type Output = Self;
    /// Returns the inverted bitmask.
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

/// Checks whether an arbitrary integer bitmask contains the required [`Permission`].
///
/// Returns `true` if `(mask & required) != 0`, matching the behavior of the
/// database SQL helper function `has_perm(mask, required)`.
#[inline]
pub fn has_perm(mask: i64, required: Permission) -> bool {
    (mask & required.as_i64()) != 0
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct TaskMessage {
    pub task_id: i64,
    pub msg_id: i64,
    pub author_id: Option<i64>,
    pub author_name: String,
    pub created_at: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OrganizationMember {
    pub organization_id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub user_email: String,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrganizationPendingInvite {
    pub organization_id: i64,
    pub invited_user_id: i64,
    pub invited_user_name: String,
    pub invited_user_email: String,
    pub role: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
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
    pub permissions_mask: i64,
    pub active_grant_key: Option<String>,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectOrganization {
    pub project_id: i64,
    pub organization_id: i64,
    pub organization_name: String,
    pub relationship_type: String,
    pub max_granted_role: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectMemberGrant {
    pub project_id: i64,
    pub user_id: i64,
    pub grant_key: String,
    pub project_role: String,
    pub source_org_id: Option<i64>,
    pub source_org_name: Option<String>,
    pub source_org_role: Option<String>,
    pub granted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectOrgRoleRule {
    pub project_id: i64,
    pub organization_id: i64,
    pub org_role: String,
    pub project_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActiveTaskImpact {
    pub project_id: i64,
    pub project_name: String,
    pub task_id: i64,
    pub task_title: String,
    pub task_status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrgProjectAffiliation {
    pub project_id: i64,
    pub project_name: String,
    pub relationship_type: String,
    pub max_granted_role: String,
    pub has_access: bool,
    pub user_role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrgRoleRuleItem {
    pub project_id: i64,
    pub project_name: String,
    pub organization_id: i64,
    pub org_role: String,
    pub project_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrganizationSummary {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectInviteCandidate {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub organization_ids: Vec<i64>,
    pub organizations_display: String,
    pub is_member: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectInviteDirectory {
    pub organizations: Vec<OrganizationSummary>,
    pub candidates: Vec<ProjectInviteCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectInvitation {
    pub project_id: i64,
    pub project_name: String,
    pub invited_user_id: i64,
    pub invited_by: Option<i64>,
    pub invited_by_name: String,
    pub role: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrganizationInvitation {
    pub organization_id: i64,
    pub organization_name: String,
    pub invited_user_id: i64,
    pub invited_by: Option<i64>,
    pub invited_by_name: String,
    pub role: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct GovernanceTransferOffer {
    pub organization_id: i64,
    pub organization_name: String,
    pub requested_new_governor: i64,
    pub current_governor_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectLeadershipTransferOffer {
    pub project_id: i64,
    pub project_name: String,
    pub requested_new_leader: i64,
    pub current_leader_name: String,
    pub created_at: DateTime<Utc>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_email() {
        // Valid email formats
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("jane.doe@sub.domain.org"));
        assert!(is_valid_email("  alice@company.co.uk  "));

        // Custom TLD & boundary rules
        assert!(!is_valid_email("user@example"), "Requires a dot-separated domain");
        assert!(!is_valid_email("user@example."), "Trailing dot should be invalid");
        assert!(!is_valid_email("user@example.c"), "TLD must be at least 2 characters");
        assert!(!is_valid_email("user@.example.com"), "Domain cannot start with dot");
        assert!(!is_valid_email("plainaddress"), "Missing @ and domain");
    }
}


