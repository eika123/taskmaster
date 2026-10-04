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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    pub title: String,
    pub author: i64,
    pub author_name: String,
    pub description: String,
    pub status: TaskStatus,
    pub assignees: Vec<User>,
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
