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
    /// Moves the user may make on the board. Everything else goes through the agent manager
    /// (sending a Done ticket back needs a comment, so it is not a plain move).
    pub fn user_can_move_to(self, to: TicketStatus) -> bool {
        use TicketStatus::*;
        self == to || matches!((self, to), (Planning, Ready) | (Ready, Planning))
    }

    /// The agent is running or waiting for the user; the project's slot is taken.
    pub fn is_active(self) -> bool {
        matches!(self, TicketStatus::InProgress | TicketStatus::NeedsInput)
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
    /// The agent that owns `agent_session_id`.
    pub agent: Option<String>,
    pub pending_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One entry of a ticket's transcript.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TicketEvent {
    pub id: i64,
    pub ticket_id: i64,
    pub seq: i64,
    pub created_at: DateTime<Utc>,
    pub data: EventData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum EventData {
    /// A prompt sent to the agent: the ticket body, a reply or a comment.
    UserPrompt {
        text: String,
    },
    AssistantText {
        text: String,
    },
    Thinking {
        text: String,
    },
    /// `summary` is a short human-readable form (command, file path, …); `input` is pretty JSON.
    ToolUse {
        id: String,
        name: String,
        summary: String,
        input: String,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    /// The agent asked the user via the `ask_user` tool, or ended its turn blocked on the user.
    Question {
        question: String,
        options: Vec<String>,
    },
    /// End of an agent turn.
    TurnResult {
        is_error: bool,
        text: Option<String>,
        cost_usd: f64,
        input_tokens: i64,
        output_tokens: i64,
        duration_ms: i64,
    },
    Error {
        message: String,
    },
    /// Notes from Workzone itself (started, cancelled, interrupted, …).
    System {
        message: String,
    },
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
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,
    pub palette: String,
    /// Agent for new sessions: `claude` or `opencode`.
    pub agent: String,
    /// Paths of the agent executables; empty means "find it on PATH".
    pub claude_path: String,
    pub opencode_path: String,
    /// Models passed to the agents (e.g. `sonnet`, `opencode/big-pickle`); empty means the
    /// agent's own default.
    #[serde(alias = "model")]
    pub claude_model: String,
    pub opencode_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "midnight".into(),
            palette: "vivid".into(),
            agent: "claude".into(),
            claude_path: String::new(),
            opencode_path: String::new(),
            claude_model: String::new(),
            opencode_model: String::new(),
        }
    }
}

/// Plan usage window reported by Claude Code (`rate_limit_event`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    /// Share of the window's limit used, 0.0–1.0.
    pub utilization: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimits {
    /// Claude's 5-hour session window.
    pub five_hour: Option<UsageWindow>,
    pub seven_day: Option<UsageWindow>,
    /// When Claude last reported these numbers.
    pub reported_at: Option<DateTime<Utc>>,
}

/// What the status bar shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub limits: UsageLimits,
    /// Totals of the last 7 days, summed from finished agent turns.
    pub week_input_tokens: i64,
    pub week_output_tokens: i64,
    pub week_cost_usd: f64,
}

/// How to run an agent, taken from the settings at the start of each run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentOptions {
    pub agent: String,
    pub binary: Option<std::path::PathBuf>,
    pub model: Option<String>,
}

impl AgentOptions {
    pub fn for_agent(s: &Settings, agent: &str) -> Self {
        let set = |v: &str| Some(v.trim()).filter(|v| !v.is_empty()).map(String::from);
        let (path, model) = match agent {
            "opencode" => (&s.opencode_path, &s.opencode_model),
            _ => (&s.claude_path, &s.claude_model),
        };
        Self { agent: agent.into(), binary: set(path).map(Into::into), model: set(model) }
    }
}
