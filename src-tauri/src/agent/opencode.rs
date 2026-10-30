//! OpenCode backend: drives `opencode run --format json`, one process per turn.
//! Messages sent while it works can't reach it mid-turn; they start the next turn instead.
//!
//! The prompt goes in on stdin (so no shell quoting is involved), `--session` continues a
//! conversation, and our `ask_user` MCP server plus the standing instructions are passed via
//! `OPENCODE_CONFIG_CONTENT`, which OpenCode merges into the user's own configuration.
//! Each output line is an event with a `sessionID`: `step_start`, `text`, `reasoning`,
//! `tool_use` (sent once the call finished), `step_finish` (tokens, cost) and `error`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use super::process::{self, Spawned};
use super::{
    AgentBackend, AgentEvent, INSTRUCTIONS, MAX_SUMMARY, MAX_TOOL_OUTPUT, Run, RunRequest, mcp, truncate,
};
use crate::attachments;
use crate::error::AppResult;
use crate::models::EventData;

const LABEL: &str = "OpenCode";
/// Name under which OpenCode exposes our MCP tool to the model.
const ASK_TOOL: &str = "hivemind_ask_user";

pub struct OpenCodeBackend {
    config: String,
}

impl OpenCodeBackend {
    /// `mcp_exe` is the Hivemind binary that serves `ask_user` (normally the running app).
    pub fn new(data_dir: &Path, mcp_exe: &Path) -> std::io::Result<Self> {
        let instructions = data_dir.join("opencode-instructions.md");
        std::fs::write(&instructions, INSTRUCTIONS)?;
        let config = json!({
            "mcp": { mcp::SERVER_NAME: {
                "type": "local",
                "command": [mcp_exe, mcp::SUBCOMMAND],
                "enabled": true,
            }},
            "instructions": [instructions],
        });
        Ok(Self { config: config.to_string() })
    }

    fn command(&self, request: &RunRequest) -> AppResult<tokio::process::Command> {
        let mut cmd = process::base_command(&find(request.options.binary.as_deref())?);
        // OpenCode takes its working directory from `--dir`/`$PWD`, not from the process cwd.
        cmd.args(["run", "--format", "json", "--dangerously-skip-permissions", "--dir"])
            .arg(&request.cwd)
            .env("PWD", &request.cwd)
            .env("OPENCODE_CONFIG_CONTENT", &self.config);
        if let Some(model) =
            request.options.model.clone().or_else(|| std::env::var("HIVEMIND_OPENCODE_MODEL").ok())
        {
            cmd.args(["--model", &model]);
        }
        if let Some(session) = &request.resume {
            cmd.args(["--session", session]);
        }
        for image in attachments::referenced_images(&request.prompt) {
            cmd.arg("--file").arg(image);
        }
        Ok(cmd)
    }
}

/// The configured path, else `HIVEMIND_OPENCODE_BIN`, else PATH, else the installer's location.
fn find(configured: Option<&Path>) -> AppResult<PathBuf> {
    let installed: Vec<PathBuf> = std::env::home_dir()
        .map(|h| h.join(".opencode").join("bin"))
        .into_iter()
        .flat_map(|dir| [dir.join("opencode"), dir.join("opencode.exe")])
        .collect();
    process::find_binary(
        LABEL,
        configured,
        "HIVEMIND_OPENCODE_BIN",
        "opencode",
        &installed,
        "https://opencode.ai",
    )
}

/// Runs `opencode --version` to check the setup.
pub async fn check(configured: Option<&Path>) -> AppResult<String> {
    process::version(&find(configured)?).await.map(|v| format!("OpenCode {v}"))
}

impl AgentBackend for OpenCodeBackend {
    fn start(&self, request: RunRequest) -> AppResult<Run> {
        let spawned = process::spawn(LABEL, &request.cwd, self.command(&request)?)?;
        let (tx, rx) = mpsc::channel(256);
        let (cancel_tx, cancel_rx) = oneshot::channel();
        tokio::spawn(drive(spawned, request.prompt, tx, cancel_rx));
        // The prompt is read from stdin up to EOF, so messages wait for the next turn.
        Ok(Run { events: rx, cancel: cancel_tx, messages: None })
    }
}

async fn drive(
    spawned: Spawned,
    prompt: String,
    tx: mpsc::Sender<AgentEvent>,
    mut cancel: oneshot::Receiver<oneshot::Sender<()>>,
) {
    let Spawned { mut child, mut stdin, stdout, stderr } = spawned;
    let started = Instant::now();
    if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
        let _ = tx.send(AgentEvent::Exited { error: Some(format!("could not send the prompt: {e}")) }).await;
        process::kill_tree(&mut child).await;
        return;
    }
    drop(stdin); // end of the prompt

    let mut parser = EventParser::default();
    let mut lines = BufReader::new(stdout).lines();
    loop {
        tokio::select! {
            ack = &mut cancel => {
                process::kill_tree(&mut child).await;
                if let Ok(ack) = ack {
                    let _ = ack.send(());
                }
                return; // a cancelled run reports nothing more
            }
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
                    for event in parser.parse(&value) {
                        let _ = tx.send(event).await;
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }
    }
    let status = child.wait().await;
    let _ = tx.send(parser.turn_result(started.elapsed().as_millis() as i64)).await;
    let error = match (parser.error.take(), status) {
        (Some(message), _) => Some(message),
        (None, Ok(s)) if s.success() => None,
        (None, other) => Some(stderr.failure(LABEL, other)),
    };
    let _ = tx.send(AgentEvent::Exited { error }).await;
}

/// Turns OpenCode's JSON events into agent events; sums up the turn's usage.
#[derive(Default)]
pub struct EventParser {
    session: Option<String>,
    last_text: Option<String>,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
    error: Option<String>,
}

impl EventParser {
    pub fn parse(&mut self, v: &Value) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        if self.session.is_none()
            && let Some(id) = v["sessionID"].as_str()
        {
            self.session = Some(id.into());
            out.push(AgentEvent::Session(id.into()));
        }
        let part = &v["part"];
        match v["type"].as_str().unwrap_or_default() {
            "text" => {
                let text = part["text"].as_str().unwrap_or_default().to_string();
                if !text.trim().is_empty() {
                    self.last_text = Some(text.clone());
                    out.push(AgentEvent::Transcript(EventData::AssistantText { text }));
                }
            }
            "reasoning" => {
                let text = part["text"].as_str().unwrap_or_default().to_string();
                if !text.trim().is_empty() {
                    out.push(AgentEvent::Transcript(EventData::Thinking { text }));
                }
            }
            "tool_use" => out.extend(tool_use(part).into_iter().map(AgentEvent::Transcript)),
            "step_finish" => {
                let tokens = &part["tokens"];
                let n = |v: &Value| v.as_i64().unwrap_or(0);
                self.input_tokens +=
                    n(&tokens["input"]) + n(&tokens["cache"]["read"]) + n(&tokens["cache"]["write"]);
                self.output_tokens += n(&tokens["output"]) + n(&tokens["reasoning"]);
                self.cost_usd += part["cost"].as_f64().unwrap_or(0.0);
            }
            "error" => {
                let e = &v["error"];
                let message =
                    e["data"]["message"].as_str().or(e["message"].as_str()).unwrap_or("unknown error");
                let name = e["name"].as_str().unwrap_or("Error");
                self.error = Some(format!("{name}: {message}"));
            }
            _ => {}
        }
        out
    }

    /// OpenCode has no end-of-turn event; this summarises the turn once the process exits.
    fn turn_result(&self, duration_ms: i64) -> AgentEvent {
        AgentEvent::Transcript(EventData::TurnResult {
            is_error: self.error.is_some(),
            text: self.last_text.clone(),
            cost_usd: self.cost_usd,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            duration_ms,
        })
    }
}

/// A finished tool call: our `ask_user` becomes a question, anything else a call plus its result.
fn tool_use(part: &Value) -> Vec<EventData> {
    let name = part["tool"].as_str().unwrap_or_default().to_string();
    let state = &part["state"];
    let input = &state["input"];
    if name == ASK_TOOL {
        let options = input["options"].as_array().into_iter().flatten().filter_map(|o| o.as_str());
        let question = input["question"].as_str().unwrap_or_default().to_string();
        return vec![EventData::Question { question, options: options.map(String::from).collect() }];
    }
    let id = part["callID"].as_str().or(part["id"].as_str()).unwrap_or_default().to_string();
    let summary = state["title"]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(String::from)
        .or_else(|| input.as_object()?.values().find_map(|v| v.as_str()).map(String::from))
        .unwrap_or_default();
    let is_error = state["status"] == "error";
    let content = if is_error { &state["error"] } else { &state["output"] };
    vec![
        EventData::ToolUse {
            id: id.clone(),
            name,
            summary: truncate(summary.lines().next().unwrap_or_default(), MAX_SUMMARY),
            input: serde_json::to_string_pretty(input).unwrap_or_default(),
        },
        EventData::ToolResult {
            tool_use_id: id,
            content: truncate(content.as_str().unwrap_or_default(), MAX_TOOL_OUTPUT),
            is_error,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(p: &mut EventParser, lines: &[&str]) -> Vec<AgentEvent> {
        lines.iter().flat_map(|l| p.parse(&serde_json::from_str(l).unwrap())).collect()
    }

    // Shapes taken from real `opencode run --format json` output (1.19.5).
    #[test]
    fn parses_tools_questions_and_usage() {
        let mut p = EventParser::default();
        let events = parse_all(
            &mut p,
            &[
                r#"{"type":"step_start","sessionID":"ses_1","part":{"type":"step-start"}}"#,
                r#"{"type":"tool_use","sessionID":"ses_1","part":{"type":"tool","tool":"bash","callID":"c1","state":{"status":"completed","input":{"command":"ls -la","description":"List"},"output":"a.txt","title":"ls -la"}}}"#,
                r#"{"type":"tool_use","sessionID":"ses_1","part":{"type":"tool","tool":"read","callID":"c2","state":{"status":"error","input":{"filePath":"/x"},"error":"File not found"}}}"#,
                r#"{"type":"step_finish","sessionID":"ses_1","part":{"type":"step-finish","reason":"tool-calls","tokens":{"input":100,"output":10,"reasoning":5,"cache":{"read":50,"write":0}},"cost":0.01}}"#,
                r#"{"type":"tool_use","sessionID":"ses_1","part":{"type":"tool","tool":"hivemind_ask_user","callID":"c3","state":{"status":"completed","input":{"question":"A or B?","options":["A","B"]},"output":"Question delivered"}}}"#,
                r#"{"type":"text","sessionID":"ses_1","part":{"type":"text","text":"Waiting for your answer."}}"#,
                r#"{"type":"step_finish","sessionID":"ses_1","part":{"type":"step-finish","reason":"stop","tokens":{"input":20,"output":3,"reasoning":0,"cache":{"read":0,"write":7}},"cost":0.002}}"#,
            ],
        );
        assert_eq!(
            events,
            vec![
                AgentEvent::Session("ses_1".into()),
                AgentEvent::Transcript(EventData::ToolUse {
                    id: "c1".into(),
                    name: "bash".into(),
                    summary: "ls -la".into(),
                    input: "{\n  \"command\": \"ls -la\",\n  \"description\": \"List\"\n}".into(),
                }),
                AgentEvent::Transcript(EventData::ToolResult {
                    tool_use_id: "c1".into(),
                    content: "a.txt".into(),
                    is_error: false
                }),
                AgentEvent::Transcript(EventData::ToolUse {
                    id: "c2".into(),
                    name: "read".into(),
                    summary: "/x".into(),
                    input: "{\n  \"filePath\": \"/x\"\n}".into(),
                }),
                AgentEvent::Transcript(EventData::ToolResult {
                    tool_use_id: "c2".into(),
                    content: "File not found".into(),
                    is_error: true
                }),
                AgentEvent::Transcript(EventData::Question {
                    question: "A or B?".into(),
                    options: vec!["A".into(), "B".into()]
                }),
                AgentEvent::Transcript(EventData::AssistantText { text: "Waiting for your answer.".into() }),
            ]
        );
        let AgentEvent::Transcript(EventData::TurnResult {
            input_tokens, output_tokens, cost_usd, text, ..
        }) = p.turn_result(1000)
        else {
            panic!("expected a turn result");
        };
        assert_eq!((input_tokens, output_tokens), (177, 18));
        assert!((cost_usd - 0.012).abs() < 1e-9);
        assert_eq!(text.as_deref(), Some("Waiting for your answer."));
    }

    #[test]
    fn errors_are_kept_for_the_end_of_the_run() {
        let mut p = EventParser::default();
        let events = parse_all(
            &mut p,
            &[
                r#"{"type":"error","sessionID":"ses_2","error":{"name":"UnknownError","data":{"message":"Unexpected server error."}}}"#,
            ],
        );
        assert_eq!(events, vec![AgentEvent::Session("ses_2".into())]);
        assert_eq!(p.error.as_deref(), Some("UnknownError: Unexpected server error."));
        assert!(matches!(
            p.turn_result(0),
            AgentEvent::Transcript(EventData::TurnResult { is_error: true, .. })
        ));
    }
}
