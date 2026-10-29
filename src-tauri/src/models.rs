use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub color_index: i64,
    pub hidden: bool,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NewProject {
    pub name: String,
    pub path: String,
    /// Picks the next unused palette slot when omitted.
    pub color_index: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub path: Option<String>,
    pub color_index: Option<i64>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum TicketStatus {
    Planning,
    Ready,
    InProgress,
    NeedsInput,
    Done,
}

impl TicketStatus {
    /// Moves the user may make by hand. Everything else is driven by the agent runner.
    pub fn user_can_move_to(self, to: TicketStatus) -> bool {
        use TicketStatus::*;
        self == to || matches!((self, to), (Planning, Ready) | (Ready, Planning) | (Done, Ready))
    }
}

#[derive(Debug, Clone, Serialize, specta::Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub body: String,
    pub status: TicketStatus,
    pub position: f64,
    pub agent_session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NewTicket {
    pub project_id: i64,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TicketPatch {
    pub title: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: String,
    pub palette: String,
    pub agent: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: "midnight".into(), palette: "vivid".into(), agent: "claude".into() }
    }
}
