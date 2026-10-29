//! Claude Code backend: drives `claude -p` in bidirectional stream-JSON mode.
//! See `spikes/agent-spike/FINDINGS.md` for the protocol details this relies on.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use command_group::AsyncCommandGroup;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use super::{AgentBackend, AgentEvent, Run, RunRequest, mcp, process};
use crate::error::{AppError, AppResult};
use crate::models::{EventData, UsageLimits, UsageWindow};

const SYSTEM_PROMPT: &str = "You are running unattended inside Workzone, a task board. Nobody watches \
your output live. When you need a decision or information from the user, call the `ask_user` tool \
with your question (and answer options if useful), then end your turn immediately. Do not ask \
questions in plain text. When you are done, end with a short summary of what you changed.";

/// Tool output beyond this many bytes is cut off in the transcript.
const MAX_TOOL_OUTPUT: usize = 8_000;
const MAX_SUMMARY: usize = 200;

pub struct ClaudeBackend {
    /// Written once at start-up; points Claude at our own binary as the `ask_user` MCP server.
    mcp_config: PathBuf,
}

impl ClaudeBackend {
    /// `mcp_exe` is the Workzone binary that serves `ask_user` (normally the running app).
    pub fn new(data_dir: &Path, mcp_exe: &Path) -> std::io::Result<Self> {
        let config = json!({ "mcpServers": { mcp::SERVER_NAME: {
            "type": "stdio",
            "command": mcp_exe,
            "args": [mcp::SUBCOMMAND],
        }}});
        let mcp_config = data_dir.join("mcp-config.json");
        std::fs::write(&mcp_config, serde_json::to_vec_pretty(&config)?)?;
        Ok(Self { mcp_config })
    }

    fn command(&self, request: &RunRequest) -> AppResult<tokio::process::Command> {
        let mut cmd = base_command(&find_claude(request.options.binary.as_deref())?);
        cmd.current_dir(&request.cwd)
            .args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"])
            .args(["--permission-mode", "bypassPermissions", "--strict-mcp-config", "--mcp-config"])
            .arg(&self.mcp_config)
            .args(["--append-system-prompt", SYSTEM_PROMPT]);
        if let Some(model) =
            request.options.model.clone().or_else(|| std::env::var("WORKZONE_CLAUDE_MODEL").ok())
        {
            cmd.args(["--model", &model]);
        }
        if let Some(session) = &request.resume {
            cmd.args(["--resume", session]);
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        Ok(cmd)
    }
}

fn base_command(binary: &Path) -> tokio::process::Command {
    // npm installs a `claude.cmd` shim on Windows, which can only run through cmd.exe.
    let is_script =
        binary.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if is_script {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(binary);
        c
    } else {
        tokio::process::Command::new(binary)
    }
}

/// The configured path, else `WORKZONE_CLAUDE_BIN`, else `claude` from PATH.
fn find_claude(configured: Option<&Path>) -> AppResult<PathBuf> {
    if let Some(path) = configured {
        return if path.is_file() {
            Ok(path.into())
        } else {
            Err(AppError::Invalid { message: format!("Claude Code not found at {}", path.display()) })
        };
    }
    if let Some(bin) = std::env::var_os("WORKZONE_CLAUDE_BIN") {
        return Ok(bin.into());
    }
    which::which("claude").map_err(|_| AppError::Invalid {
        message: "Claude Code was not found on PATH. Install it (https://claude.com/claude-code) \
                  or set its path in Settings."
            .into(),
    })
}

/// Runs `claude --version` to check the setup; returns the version line.
pub async fn check(configured: Option<&Path>) -> AppResult<String> {
    let binary = find_claude(configured)?;
    let mut cmd = base_command(&binary);
    cmd.arg("--version");
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = tokio::time::timeout(std::time::Duration::from_secs(15), cmd.output())
        .await
        .map_err(|_| AppError::Invalid { message: "`claude --version` did not answer".into() })?
        .map_err(|e| AppError::Invalid { message: format!("could not run {}: {e}", binary.display()) })?;
    let text = String::from_utf8_lossy(if out.status.success() { &out.stdout } else { &out.stderr });
    let line = text.lines().next().unwrap_or_default().trim().to_string();
    if out.status.success() {
        Ok(format!("{line} ({})", binary.display()))
    } else {
        Err(AppError::Invalid { message: format!("{} failed: {line}", binary.display()) })
    }
}

impl AgentBackend for ClaudeBackend {
    fn start(&self, request: RunRequest) -> AppResult<Run> {
        if !request.cwd.is_dir() {
            return Err(AppError::Invalid {
                message: format!("project folder not found: {}", request.cwd.display()),
            });
        }
        let mut cmd = self.command(&request)?;
        let mut group = cmd.group();
        #[cfg(windows)]
        group.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = group
            .spawn()
            .map_err(|e| AppError::Invalid { message: format!("could not start Claude Code: {e}") })?;
        let stdin = child.inner().stdin.take().expect("stdin is piped");
        let stdout = child.inner().stdout.take().expect("stdout is piped");
        let stderr = child.inner().stderr.take().expect("stderr is piped");

        let (tx, rx) = mpsc::channel(256);
        let (cancel_tx, cancel_rx) = oneshot::channel();
        tokio::spawn(drive(child, stdin, stdout, stderr, request.prompt, tx, cancel_rx));
        Ok(Run { events: rx, cancel: cancel_tx })
    }
}

async fn drive(
    mut child: command_group::AsyncGroupChild,
    mut stdin: tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    prompt: String,
    tx: mpsc::Sender<AgentEvent>,
    mut cancel: oneshot::Receiver<oneshot::Sender<()>>,
) {
    // Keep the last lines of stderr for error messages.
    let stderr_tail = Arc::new(Mutex::new(VecDeque::<String>::new()));
    let tail = stderr_tail.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut t = tail.lock().unwrap();
            t.push_back(line);
            if t.len() > 20 {
                t.pop_front();
            }
        }
    });

    let message = json!({ "type": "user", "message": { "role": "user", "content": prompt } });
    let mut stdin = match stdin.write_all(format!("{message}\n").as_bytes()).await {
        Ok(()) => Some(stdin),
        Err(e) => {
            let _ =
                tx.send(AgentEvent::Exited { error: Some(format!("could not send the prompt: {e}")) }).await;
            process::kill_tree(&mut child).await;
            return;
        }
    };

    let mut parser = StreamParser::default();
    let mut saw_result = false;
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
                    if value["type"] == "result" {
                        saw_result = true;
                        stdin = None; // one turn per process: closing stdin lets the CLI exit
                    }
                    for event in parser.parse(&value) {
                        let _ = tx.send(event).await;
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }
    }
    drop(stdin);
    let status = child.wait().await;
    let error = match status {
        Ok(s) if s.success() && saw_result => None,
        other => {
            let tail: Vec<String> = stderr_tail.lock().unwrap().iter().cloned().collect();
            let reason = match other {
                Ok(s) => format!("Claude Code exited unexpectedly ({s})"),
                Err(e) => format!("Claude Code failed: {e}"),
            };
            Some(if tail.is_empty() { reason } else { format!("{reason}:\n{}", tail.join("\n")) })
        }
    };
    let _ = tx.send(AgentEvent::Exited { error }).await;
}

/// Turns stream-JSON messages into agent events.
#[derive(Default)]
pub struct StreamParser {
    /// `tool_use` ids of `ask_user` calls; their (canned) results are not worth showing.
    ask_ids: HashSet<String>,
}

impl StreamParser {
    pub fn parse(&mut self, v: &Value) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        match (v["type"].as_str().unwrap_or_default(), v["subtype"].as_str().unwrap_or_default()) {
            ("system", "init") => {
                if let Some(id) = v["session_id"].as_str() {
                    out.push(AgentEvent::Session(id.into()));
                }
            }
            ("system", "post_turn_summary") if v["status_category"] == "blocked" => {
                let needs_action = v["needs_action"].as_str().filter(|s| !s.is_empty()).map(String::from);
                out.push(AgentEvent::Blocked { needs_action });
            }
            ("assistant", _) => {
                for block in blocks(v) {
                    match block["type"].as_str() {
                        Some("text") => {
                            let text = str_of(&block["text"]);
                            if !text.trim().is_empty() {
                                out.push(AgentEvent::Transcript(EventData::AssistantText { text }));
                            }
                        }
                        Some("thinking") => {
                            let text = str_of(&block["thinking"]);
                            if !text.trim().is_empty() {
                                out.push(AgentEvent::Transcript(EventData::Thinking { text }));
                            }
                        }
                        Some("tool_use") => out.push(AgentEvent::Transcript(self.tool_use(block))),
                        _ => {}
                    }
                }
            }
            ("user", _) => {
                for block in blocks(v).filter(|b| b["type"] == "tool_result") {
                    let tool_use_id = str_of(&block["tool_use_id"]);
                    if self.ask_ids.contains(&tool_use_id) {
                        continue;
                    }
                    out.push(AgentEvent::Transcript(EventData::ToolResult {
                        tool_use_id,
                        content: truncate(&tool_result_text(&block["content"]), MAX_TOOL_OUTPUT),
                        is_error: block["is_error"].as_bool().unwrap_or(false),
                    }));
                }
            }
            ("rate_limit_event", _) => {
                let windows = &v["rate_limit_info"]["unifiedWindows"];
                let window = |w: &Value| {
                    Some(UsageWindow {
                        utilization: w["utilization"].as_f64()?,
                        resets_at: w["resetsAt"]
                            .as_i64()
                            .and_then(|t| chrono::DateTime::from_timestamp(t, 0)),
                    })
                };
                out.push(AgentEvent::Limits(UsageLimits {
                    five_hour: window(&windows["five_hour"]),
                    seven_day: window(&windows["seven_day"]),
                    reported_at: Some(chrono::Utc::now()),
                }));
            }
            ("result", _) => {
                let usage = &v["usage"];
                let tokens = |k: &str| usage[k].as_i64().unwrap_or(0);
                out.push(AgentEvent::Transcript(EventData::TurnResult {
                    is_error: v["is_error"].as_bool().unwrap_or(false),
                    text: v["result"].as_str().map(String::from),
                    cost_usd: v["total_cost_usd"].as_f64().unwrap_or(0.0),
                    input_tokens: tokens("input_tokens")
                        + tokens("cache_creation_input_tokens")
                        + tokens("cache_read_input_tokens"),
                    output_tokens: tokens("output_tokens"),
                    duration_ms: v["duration_ms"].as_i64().unwrap_or(0),
                }));
            }
            _ => {}
        }
        out
    }

    fn tool_use(&mut self, block: &Value) -> EventData {
        let id = str_of(&block["id"]);
        let name = str_of(&block["name"]);
        let input = &block["input"];
        if name == mcp::QUALIFIED_TOOL_NAME {
            self.ask_ids.insert(id);
            let options = input["options"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|o| o.as_str())
                .map(String::from);
            return EventData::Question { question: str_of(&input["question"]), options: options.collect() };
        }
        EventData::ToolUse {
            summary: truncate(&tool_summary(&name, input), MAX_SUMMARY),
            input: serde_json::to_string_pretty(input).unwrap_or_default(),
            id,
            name,
        }
    }
}

fn blocks(v: &Value) -> impl Iterator<Item = &Value> {
    v["message"]["content"].as_array().into_iter().flatten()
}

fn str_of(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

/// The most telling input field of common tools, for a one-line display.
fn tool_summary(name: &str, input: &Value) -> String {
    let field = match name {
        "Bash" => "command",
        "Read" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => "file_path",
        "Glob" | "Grep" => "pattern",
        "WebFetch" => "url",
        "WebSearch" => "query",
        "Task" | "Agent" => "description",
        _ => "",
    };
    if let Some(s) = input[field].as_str() {
        return s.lines().next().unwrap_or_default().to_string();
    }
    // Otherwise the first string field, if any.
    input.as_object().and_then(|o| o.values().find_map(|v| v.as_str())).unwrap_or_default().to_string()
}

fn tool_result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let cut = (0..=max).rev().find(|i| s.is_char_boundary(*i)).unwrap_or(0);
    format!("{}… [{} more bytes]", &s[..cut], s.len() - cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(lines: &[&str]) -> Vec<AgentEvent> {
        let mut p = StreamParser::default();
        let mut events: Vec<_> =
            lines.iter().flat_map(|l| p.parse(&serde_json::from_str(l).unwrap())).collect();
        for e in &mut events {
            if let AgentEvent::Limits(l) = e {
                l.reported_at = None;
            }
        }
        events
    }

    #[test]
    fn parses_a_turn_with_tools_and_a_question() {
        let events = parse_all(&[
            r#"{"type":"system","subtype":"init","session_id":"s-1","tools":[]}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"Looking."},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls -la\nmore","description":"List"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"a.txt"}],"is_error":false}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"mcp__workzone__ask_user","input":{"question":"A or B?","options":["A","B"]}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t2","content":"Question delivered"}]}}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.25,"resetsAt":1793307000}}}}"#,
            r#"{"type":"system","subtype":"post_turn_summary","status_category":"blocked","needs_action":"choose A or B"}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"Asked.","total_cost_usd":0.01,"duration_ms":1500,"usage":{"input_tokens":2,"cache_creation_input_tokens":100,"cache_read_input_tokens":50,"output_tokens":7}}"#,
        ]);
        assert_eq!(
            events,
            vec![
                AgentEvent::Session("s-1".into()),
                AgentEvent::Transcript(EventData::Thinking { text: "hmm".into() }),
                AgentEvent::Transcript(EventData::AssistantText { text: "Looking.".into() }),
                AgentEvent::Transcript(EventData::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    summary: "ls -la".into(),
                    input: "{\n  \"command\": \"ls -la\\nmore\",\n  \"description\": \"List\"\n}".into(),
                }),
                AgentEvent::Transcript(EventData::ToolResult {
                    tool_use_id: "t1".into(),
                    content: "a.txt".into(),
                    is_error: false
                }),
                AgentEvent::Transcript(EventData::Question {
                    question: "A or B?".into(),
                    options: vec!["A".into(), "B".into()]
                }),
                AgentEvent::Limits(UsageLimits {
                    five_hour: Some(UsageWindow {
                        utilization: 0.25,
                        resets_at: chrono::DateTime::from_timestamp(1793307000, 0)
                    }),
                    seven_day: None,
                    reported_at: None, // compared without the timestamp below
                }),
                AgentEvent::Blocked { needs_action: Some("choose A or B".into()) },
                AgentEvent::Transcript(EventData::TurnResult {
                    is_error: false,
                    text: Some("Asked.".into()),
                    cost_usd: 0.01,
                    input_tokens: 152,
                    output_tokens: 7,
                    duration_ms: 1500,
                }),
            ]
        );
    }

    #[test]
    fn completed_turns_are_not_blocked_and_long_output_is_truncated() {
        let long = "x".repeat(MAX_TOOL_OUTPUT + 10);
        let line = json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t","content":long}]}});
        let mut p = StreamParser::default();
        let Some(AgentEvent::Transcript(EventData::ToolResult { content, .. })) = p.parse(&line).pop() else {
            panic!("expected a tool result");
        };
        assert!(content.ends_with("[10 more bytes]"));
        let done = json!({"type":"system","subtype":"post_turn_summary","status_category":"completed"});
        assert!(p.parse(&done).is_empty());
    }
}
