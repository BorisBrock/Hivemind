//! Phase 0 spike: drive the `claude` CLI headless from Rust.
//!
//! Subcommands:
//!   run <cwd> <prompt> [resume_session_id]   run one turn, print events + outcome
//!   cancel-test <cwd> <group|term|tree>      start a long task, stop it, check for survivors
//!   mcp-ask-user                             stdio MCP server exposing `ask_user`

use anyhow::{bail, Context, Result};
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

const ASK_TOOL: &str = "mcp__workzone__ask_user";
const SYSTEM_PROMPT: &str = "You are running unattended inside the Workzone app. \
When you need a decision or information from the user, call the `ask_user` tool with your \
question, then end your turn immediately. Do not ask questions in plain text.";

#[derive(Debug, Default)]
struct TurnOutcome {
    session_id: Option<String>,
    ask_user_question: Option<String>,
    summary_category: Option<String>,
    summary_needs_action: Option<String>,
    result_text: Option<String>,
    is_error: bool,
    output_tokens: u64,
    cost_usd: f64,
    five_hour_util: Option<f64>,
    seven_day_util: Option<f64>,
}

impl TurnOutcome {
    fn column(&self) -> &'static str {
        if self.is_error {
            "Done (error)"
        } else if self.ask_user_question.is_some() {
            "Needs Input (ask_user tool)"
        } else if self.summary_category.as_deref() == Some("blocked") {
            "Needs Input (post_turn_summary)"
        } else {
            "Done"
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") if args.len() >= 4 => {
            let outcome = run_turn(&args[2], &args[3], args.get(4).map(String::as_str)).await?;
            println!("\n=== outcome ===\n{outcome:#?}\n=> column: {}", outcome.column());
            Ok(())
        }
        Some("cancel-test") if args.len() >= 4 => cancel_test(&args[2], &args[3]).await,
        Some("mcp-ask-user") => mcp_ask_user().await,
        _ => bail!("usage: agent-spike run <cwd> <prompt> [session] | cancel-test <cwd> <group|term|tree> | mcp-ask-user"),
    }
}

fn claude_command(cwd: &str, resume: Option<&str>) -> Result<Command> {
    let exe = std::env::current_exe()?;
    let mcp_config = json!({
        "mcpServers": { "workzone": {
            "type": "stdio",
            "command": exe.to_string_lossy(),
            "args": ["mcp-ask-user"],
        }}
    });
    let mut cmd = Command::new(std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into()));
    cmd.current_dir(cwd)
        .args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"])
        .args(["--permission-mode", "bypassPermissions"])
        .args(["--model", &std::env::var("CLAUDE_MODEL").unwrap_or_else(|_| "haiku".into())])
        .args(["--strict-mcp-config", "--mcp-config", &mcp_config.to_string()])
        .args(["--append-system-prompt", SYSTEM_PROMPT]);
    if let Some(id) = resume {
        cmd.args(["--resume", id]);
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    Ok(cmd)
}

async fn spawn(cwd: &str, prompt: &str, resume: Option<&str>) -> Result<AsyncGroupChild> {
    let mut child = claude_command(cwd, resume)?.group_spawn().context("spawning claude")?;
    let msg = json!({"type": "user", "message": {"role": "user", "content": prompt}});
    let stdin = child.inner().stdin.as_mut().context("no stdin")?;
    stdin.write_all(format!("{msg}\n").as_bytes()).await?;
    stdin.flush().await?;
    Ok(child)
}

async fn run_turn(cwd: &str, prompt: &str, resume: Option<&str>) -> Result<TurnOutcome> {
    let mut child = spawn(cwd, prompt, resume).await?;
    let stdout = child.inner().stdout.take().context("no stdout")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut out = TurnOutcome::default();

    while let Some(line) = lines.next_line().await? {
        let Ok(ev) = serde_json::from_str::<Value>(&line) else {
            println!("[unparsed] {line}");
            continue;
        };
        let kind = ev["type"].as_str().unwrap_or("?");
        let subtype = ev["subtype"].as_str().unwrap_or("");
        match (kind, subtype) {
            ("system", "init") => {
                out.session_id = ev["session_id"].as_str().map(Into::into);
                println!("[init] session={}", ev["session_id"]);
            }
            ("system", "post_turn_summary") => {
                out.summary_category = ev["status_category"].as_str().map(Into::into);
                out.summary_needs_action =
                    ev["needs_action"].as_str().filter(|s| !s.is_empty()).map(Into::into);
                println!("[summary] {} | {}", ev["status_category"], ev["status_detail"]);
            }
            ("assistant", _) => {
                for block in ev["message"]["content"].as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => println!("[assistant] {}", block["text"].as_str().unwrap_or("")),
                        Some("thinking") => println!("[thinking] …"),
                        Some("tool_use") => {
                            println!("[tool_use] {} {}", block["name"], block["input"]);
                            if block["name"] == ASK_TOOL {
                                out.ask_user_question = block["input"]["question"].as_str().map(Into::into);
                            }
                        }
                        other => println!("[assistant:{other:?}]"),
                    }
                }
            }
            ("user", _) => println!("[tool_result]"),
            ("rate_limit_event", _) => {
                let w = &ev["rate_limit_info"]["unifiedWindows"];
                out.five_hour_util = w["five_hour"]["utilization"].as_f64();
                out.seven_day_util = w["seven_day"]["utilization"].as_f64();
            }
            ("result", _) => {
                out.is_error = ev["is_error"].as_bool().unwrap_or(false);
                out.result_text = ev["result"].as_str().map(Into::into);
                out.output_tokens = ev["usage"]["output_tokens"].as_u64().unwrap_or(0);
                out.cost_usd = ev["total_cost_usd"].as_f64().unwrap_or(0.0);
                // One turn per process: closing stdin lets the CLI exit.
                drop(child.inner().stdin.take());
            }
            _ => println!("[{kind}:{subtype}]"),
        }
    }
    let status = child.wait().await?;
    println!("[exit] {status}");
    Ok(out)
}

async fn cancel_test(cwd: &str, mode: &str) -> Result<()> {
    let marker = format!("{}", std::process::id());
    let prompt = format!("Run this exact shell command in the foreground and wait for it: python3 -c \"import time; time.sleep(120)\" # wz-{marker}");
    let mut child = spawn(cwd, &prompt, None).await?;
    let stdout = child.inner().stdout.take().context("no stdout")?;
    let mut lines = BufReader::new(stdout).lines();

    // Wait until the agent has actually started the shell command.
    let started = tokio::time::timeout(Duration::from_secs(90), async {
        while let Some(line) = lines.next_line().await? {
            if line.contains("\"tool_use\"") && line.contains(&marker) {
                println!("started: {}", &line[..line.len().min(400)]);
                return anyhow::Ok(true);
            }
        }
        Ok(false)
    })
    .await??;
    if !started {
        bail!("agent never started the command");
    }
    tokio::time::sleep(Duration::from_secs(8)).await;
    let root = child.id().context("no pid")?;
    let tree = descendants(root);
    println!("process tree before kill ({} procs):", tree.len());
    for (pid, pgid, args) in &tree {
        println!("  pid={pid} pgid={pgid} {args}");
    }

    match mode {
        // SIGKILL to our process group (Job Object on Windows).
        "group" => child.kill().await?,
        // SIGTERM to claude only, so it can clean up its own shells.
        "term" => signal(&[root], "TERM"),
        // SIGKILL to every process in the snapshot, then the group.
        "tree" => {
            signal(&tree.iter().map(|t| t.0).collect::<Vec<_>>(), "KILL");
            child.kill().await?;
        }
        _ => bail!("unknown mode {mode}"),
    }
    child.wait().await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let alive: Vec<_> = tree.iter().filter(|(pid, ..)| pid_alive(*pid)).collect();
    println!("still alive after kill: {alive:?}");
    if !alive.is_empty() {
        bail!("process tree not fully killed");
    }
    println!("cancel OK");
    Ok(())
}

/// (pid, pgid, args) of every descendant of `root`, via `ps`.
#[cfg(unix)]
fn descendants(root: u32) -> Vec<(u32, u32, String)> {
    let out = std::process::Command::new("ps").args(["-eo", "pid=,ppid=,pgid=,args="]).output().unwrap();
    let procs: Vec<(u32, u32, u32, String)> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let pid = it.next()?.parse().ok()?;
            let ppid = it.next()?.parse().ok()?;
            let pgid = it.next()?.parse().ok()?;
            Some((pid, ppid, pgid, it.collect::<Vec<_>>().join(" ")))
        })
        .collect();
    let mut found = vec![root];
    let mut i = 0;
    while i < found.len() {
        let parent = found[i];
        found.extend(procs.iter().filter(|p| p.1 == parent).map(|p| p.0));
        i += 1;
    }
    procs.into_iter().filter(|p| found.contains(&p.0)).map(|p| (p.0, p.2, p.3)).collect()
}

fn signal(pids: &[u32], sig: &str) {
    let pids: Vec<String> = pids.iter().map(u32::to_string).collect();
    let _ = std::process::Command::new("kill").arg(format!("-{sig}")).args(&pids).status();
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).map(|s| !s.contains(") Z ")).unwrap_or(false)
}

#[cfg(windows)]
fn descendants(_root: u32) -> Vec<(u32, u32, String)> {
    // TODO(windows): enumerate via Toolhelp32; the Job Object kill is what matters.
    vec![]
}

#[cfg(windows)]
fn pid_alive(_pid: u32) -> bool {
    false
}

/// Minimal stateless MCP server. The app sees the question in the agent's
/// `tool_use` event, so the tool only has to acknowledge it.
async fn mcp_ask_user() -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = req.get("id").cloned() else { continue }; // notification
        let result = match req["method"].as_str().unwrap_or("") {
            "initialize" => json!({
                "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "workzone", "version": "0.0.1" },
            }),
            "ping" => json!({}),
            "tools/list" => json!({ "tools": [{
                "name": "ask_user",
                "description": "Ask the user a question or for a decision. After calling this, end your turn; the answer arrives as the next user message.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "question": { "type": "string" },
                        "options": { "type": "array", "items": { "type": "string" } },
                    },
                    "required": ["question"],
                },
            }]}),
            "tools/call" => json!({ "content": [{
                "type": "text",
                "text": "Question delivered to the user. End your turn now without further text; the answer will arrive as the next message.",
            }]}),
            _ => {
                let err = json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "method not found"}});
                stdout.write_all(format!("{err}\n").as_bytes()).await?;
                stdout.flush().await?;
                continue;
            }
        };
        let resp = json!({"jsonrpc": "2.0", "id": id, "result": result});
        stdout.write_all(format!("{resp}\n").as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}
