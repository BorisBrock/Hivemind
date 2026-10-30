//! Claude Code backend: drives `claude -p` in bidirectional stream-JSON mode.
//! See `spikes/agent-spike/FINDINGS.md` for the protocol details this relies on.
//!
//! Messages written to stdin while a turn runs are taken in at the next tool call; one that
//! comes too late for that starts another turn after the `result`. With
//! `--replay-user-messages`, Claude echoes each user message (`isReplay`) when it takes it in,
//! so stdin stays open until every message has been echoed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use super::process::{self, Spawned};
use super::{
    AgentBackend, AgentEvent, INSTRUCTIONS, MAX_SUMMARY, MAX_TOOL_OUTPUT, Run, RunRequest, mcp, truncate,
};
use crate::attachments;
use crate::error::AppResult;
use crate::models::{EventData, UsageLimits, UsageWindow};

const LABEL: &str = "Claude Code";

pub struct ClaudeBackend {
    /// Written once at start-up; points Claude at our own binary as the `ask_user` MCP server.
    mcp_config: PathBuf,
}

impl ClaudeBackend {
    /// `mcp_exe` is the Hivemind binary that serves `ask_user` (normally the running app).
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
        let mut cmd = process::base_command(&find(request.options.binary.as_deref())?);
        cmd.current_dir(&request.cwd)
            .args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"])
            .arg("--replay-user-messages")
            .args(["--permission-mode", "bypassPermissions", "--strict-mcp-config", "--mcp-config"])
            .arg(&self.mcp_config)
            .args(["--append-system-prompt", INSTRUCTIONS]);
        if let Some(model) =
            request.options.model.clone().or_else(|| std::env::var("HIVEMIND_CLAUDE_MODEL").ok())
        {
            cmd.args(["--model", &model]);
        }
        if let Some(session) = &request.resume {
            cmd.args(["--resume", session]);
        }
        Ok(cmd)
    }
}

/// The configured path, else `HIVEMIND_CLAUDE_BIN`, else `claude` from PATH.
fn find(configured: Option<&Path>) -> AppResult<PathBuf> {
    process::find_binary(
        LABEL,
        configured,
        "HIVEMIND_CLAUDE_BIN",
        "claude",
        &[],
        "https://claude.com/claude-code",
    )
}

/// Runs `claude --version` to check the setup; returns the version line.
pub async fn check(configured: Option<&Path>) -> AppResult<String> {
    process::version(&find(configured)?).await
}

impl AgentBackend for ClaudeBackend {
    fn start(&self, request: RunRequest) -> AppResult<Run> {
        let spawned = process::spawn(LABEL, &request.cwd, self.command(&request)?)?;
        let (tx, rx) = mpsc::channel(256);
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let (messages_tx, messages_rx) = mpsc::unbounded_channel();
        tokio::spawn(drive(spawned, request.prompt, tx, cancel_rx, messages_rx));
        Ok(Run { events: rx, cancel: cancel_tx, messages: Some(messages_tx) })
    }
}

async fn write_message(stdin: &mut tokio::process::ChildStdin, text: &str) -> std::io::Result<()> {
    let message = json!({ "type": "user", "message": { "role": "user", "content": content(text) } });
    stdin.write_all(format!("{message}\n").as_bytes()).await
}

async fn drive(
    spawned: Spawned,
    prompt: String,
    tx: mpsc::Sender<AgentEvent>,
    mut cancel: oneshot::Receiver<oneshot::Sender<()>>,
    mut messages: mpsc::UnboundedReceiver<String>,
) {
    let Spawned { mut child, stdin, stdout, stderr } = spawned;
    let mut stdin = Some(stdin);
    if let Err(e) = write_message(stdin.as_mut().unwrap(), &prompt).await {
        let _ = tx.send(AgentEvent::Exited { error: Some(format!("could not send the prompt: {e}")) }).await;
        process::kill_tree(&mut child).await;
        return;
    }

    let mut parser = StreamParser::default();
    let mut saw_result = false;
    // User messages written to stdin, and how many of them Claude has echoed (taken in).
    let (mut sent, mut taken) = (1, 0);
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
            Some(text) = messages.recv() => {
                let written = match stdin.as_mut() {
                    Some(stdin) => write_message(stdin, &text).await.is_ok(),
                    None => false,
                };
                if written {
                    sent += 1;
                } else {
                    let _ = tx.send(AgentEvent::Undelivered(text)).await;
                }
            }
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
                    if value["type"] == "user" && value["isReplay"] == true {
                        taken += 1;
                    }
                    // Closing stdin after the last turn lets the CLI exit. Without any echo
                    // (a CLI that ignores the flag), the run ends after one turn as before.
                    if value["type"] == "result" {
                        saw_result = true;
                        if taken >= sent || taken == 0 {
                            stdin = None;
                        }
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
    messages.close();
    while let Ok(text) = messages.try_recv() {
        let _ = tx.send(AgentEvent::Undelivered(text)).await;
    }
    let status = child.wait().await;
    let error = match status {
        Ok(s) if s.success() && saw_result => None,
        other => Some(stderr.failure(LABEL, other)),
    };
    let _ = tx.send(AgentEvent::Exited { error }).await;
}

/// Largest image sent inline; the API rejects bigger ones. Larger images stay available to
/// the agent through their path in the prompt.
const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024 * 3 / 4;

/// Message content: the prompt text, plus the images it references as image blocks.
fn content(prompt: &str) -> Value {
    let images: Vec<Value> = attachments::referenced_images(prompt)
        .into_iter()
        .filter(|p| std::fs::metadata(p).is_ok_and(|m| m.len() <= MAX_IMAGE_BYTES))
        .filter_map(|path| {
            let data = std::fs::read(&path).ok()?;
            Some(json!({ "type": "image", "source": {
                "type": "base64",
                "media_type": attachments::media_type(&path)?,
                "data": base64::engine::general_purpose::STANDARD.encode(data),
            }}))
        })
        .collect();
    if images.is_empty() {
        return Value::String(prompt.into());
    }
    let mut blocks = vec![json!({ "type": "text", "text": prompt })];
    blocks.extend(images);
    Value::Array(blocks)
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
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"mcp__hivemind__ask_user","input":{"question":"A or B?","options":["A","B"]}}]}}"#,
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
    fn referenced_images_are_sent_as_image_blocks() {
        assert_eq!(content("plain"), json!("plain"));
        let dir = std::env::temp_dir().join(format!("hivemind-claude-images-{}", std::process::id()));
        let png = base64::engine::general_purpose::STANDARD.encode(b"png bytes");
        let path = attachments::save(&dir, &png, "png").unwrap();
        let prompt = format!("See ![image](<{path}>)");
        assert_eq!(
            content(&prompt),
            json!([
                { "type": "text", "text": prompt },
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": png } },
            ])
        );
        std::fs::remove_dir_all(&dir).unwrap();
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
