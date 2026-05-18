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
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
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
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub published: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "project_member_role", rename_all = "snake_case")]
#[serde(rename_all = "lowercase")]
pub enum ProjectMemberRole {
    Owner,
    Editor,
    Viewer,
}

impl ProjectMemberRole {
    pub fn satisfies(&self, min: &ProjectMemberRole) -> bool {
        match (self, min) {
            (_, ProjectMemberRole::Viewer) => true,
            (ProjectMemberRole::Owner | ProjectMemberRole::Editor, ProjectMemberRole::Editor) => true,
            (ProjectMemberRole::Owner, ProjectMemberRole::Owner) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectMember {
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub role: ProjectMemberRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectInviteLink {
    pub id: Uuid,
    pub project_id: Uuid,
    pub role: ProjectMemberRole,
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
    pub rank_position: i32,
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
