//! Running coding agents headless and turning their output into ticket events.

pub mod claude;
pub mod manager;
pub mod mcp;
pub mod opencode;
pub mod process;

#[cfg(test)]
mod e2e;

use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};

use crate::error::AppResult;
use crate::models::{AgentOptions, EventData, UsageLimits};

/// Standing instructions for every agent, so it knows it runs unattended.
pub const INSTRUCTIONS: &str = "You are running unattended inside Hivemind, a task board. Nobody watches \
your output live. When you need a decision or information from the user, call the `ask_user` tool \
with your question (and answer options if useful), then end your turn immediately. Do not ask \
questions in plain text. When you are done, end with a short summary of what you changed.";

/// Tool output beyond this many bytes is cut off in the transcript.
pub const MAX_TOOL_OUTPUT: usize = 8_000;
/// Length of the one-line tool summary.
pub const MAX_SUMMARY: usize = 200;

pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let cut = (0..=max).rev().find(|i| s.is_char_boundary(*i)).unwrap_or(0);
    format!("{}… [{} more bytes]", &s[..cut], s.len() - cut)
}

/// One agent turn: start a session (or continue one) with a prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct RunRequest {
    pub cwd: PathBuf,
    pub prompt: String,
    pub resume: Option<String>,
    pub options: AgentOptions,
}

/// What a backend reports while a run is going.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// The agent's session id, needed to continue the conversation later.
    Session(String),
    Transcript(EventData),
    /// Plan usage reported by the agent.
    Limits(UsageLimits),
    /// The agent ended its turn waiting for the user (without using `ask_user`).
    Blocked {
        needs_action: Option<String>,
    },
    /// The run is over. `error` is set when the agent could not run or crashed.
    Exited {
        error: Option<String>,
    },
}

/// Handle to a running agent. Sending on `cancel` stops it; the backend answers on the
/// enclosed sender once the process tree is gone.
pub struct Run {
    pub events: mpsc::Receiver<AgentEvent>,
    pub cancel: oneshot::Sender<oneshot::Sender<()>>,
}

pub trait AgentBackend: Send + Sync {
    fn start(&self, request: RunRequest) -> AppResult<Run>;
}

/// Routes each run to the agent named in its options.
pub struct Agents {
    pub claude: claude::ClaudeBackend,
    pub opencode: opencode::OpenCodeBackend,
}

impl AgentBackend for Agents {
    fn start(&self, request: RunRequest) -> AppResult<Run> {
        match request.options.agent.as_str() {
            "opencode" => self.opencode.start(request),
            _ => self.claude.start(request),
        }
    }
}

/// Checks that an agent's CLI can be found and runs; returns its version.
pub async fn check(agent: &str, path: Option<&std::path::Path>) -> AppResult<String> {
    match agent {
        "opencode" => opencode::check(path).await,
        _ => claude::check(path).await,
    }
}
