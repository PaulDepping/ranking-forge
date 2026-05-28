use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub project_id: Uuid,
    pub params: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub progress: Option<serde_json::Value>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
    pub startgg_api_key: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Project {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Ranking {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub published: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RankingPlayer {
    pub ranking_id: Uuid,
    pub player_id: Uuid,
    pub rank_position: i32,
    pub notes: Option<String>,
}

/// DB-mapped role for project_members rows. Only editor and viewer — owner is
/// stored as projects.owner_id.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "project_member_role", rename_all = "snake_case")]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    Editor,
    Viewer,
}

/// Role returned in API responses — includes Owner (synthesised from owner_id,
/// never stored in project_members).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Owner,
    Editor,
    Viewer,
}

impl UserRole {
    pub fn satisfies(&self, min: &UserRole) -> bool {
        match (self, min) {
            (_, UserRole::Viewer) => true,
            (UserRole::Owner | UserRole::Editor, UserRole::Editor) => true,
            (UserRole::Owner, UserRole::Owner) => true,
            _ => false,
        }
    }
}

impl From<MemberRole> for UserRole {
    fn from(r: MemberRole) -> Self {
        match r {
            MemberRole::Editor => UserRole::Editor,
            MemberRole::Viewer => UserRole::Viewer,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectMember {
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectInviteLink {
    pub id: Uuid,
    pub project_id: Uuid,
    pub role: MemberRole,
    pub created_by: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Player {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct StartggAccount {
    pub id: Uuid,
    pub player_id: Uuid,
    pub startgg_user_id: i64,
    pub handle: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}
