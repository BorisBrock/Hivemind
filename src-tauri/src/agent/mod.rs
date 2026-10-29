//! Running coding agents headless and turning their output into ticket events.

pub mod claude;
pub mod manager;
pub mod mcp;
mod process;

#[cfg(test)]
mod e2e_tests;

use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};

use crate::error::AppResult;
use crate::models::{AgentOptions, EventData, UsageLimits};

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
