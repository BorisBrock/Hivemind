//! End-to-end runs against the real Claude Code CLI. They cost tokens, so they are ignored
//! by default. Run with (after `cargo build`, which provides the MCP server binary):
//!
//!     WORKZONE_CLAUDE_MODEL=haiku WORKZONE_OPENCODE_MODEL=opencode/big-pickle \
//!         cargo test agent::e2e -- --ignored --nocapture --test-threads 1
//!
//! Each scenario runs once per agent (`agent::e2e::claude::*`, `agent::e2e::opencode::*`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::Agents;
use super::claude::ClaudeBackend;
use super::manager::{AgentManager, Notifier};
use super::opencode::OpenCodeBackend;
use crate::db;
use crate::models::*;
use crate::repo;

struct PrintNotifier;
impl Notifier for PrintNotifier {
    fn ticket_changed(&self, t: &Ticket) {
        println!("  [ticket {}] {:?}", t.id, t.status);
    }
    fn event_added(&self, e: &TicketEvent) {
        let line = format!("{:?}", e.data);
        println!("  [event] {}", &line[..line.len().min(160)]);
    }
    fn usage_changed(&self, u: &Usage) {
        println!("  [usage] {u:?}");
    }
}

struct Env {
    db: sqlx::SqlitePool,
    agents: Arc<AgentManager>,
    project_dir: PathBuf,
    project_id: i64,
}

async fn env(name: &str, agent: &str) -> Env {
    let mcp_exe = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/workzone");
    assert!(mcp_exe.exists(), "run `cargo build` first: the MCP server is the app binary");
    let base = std::env::temp_dir().join(format!("workzone-e2e-{name}-{}", std::process::id()));
    let project_dir = base.join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let db = db::open_in_memory().await.unwrap();
    let backend = Arc::new(Agents {
        claude: ClaudeBackend::new(&base, &mcp_exe).unwrap(),
        opencode: OpenCodeBackend::new(&base, &mcp_exe).unwrap(),
    });
    let agents = AgentManager::new(db.clone(), backend, Arc::new(PrintNotifier));
    let settings = Settings { agent: agent.into(), ..Default::default() };
    repo::save_settings(&db, &settings).await.unwrap();
    let path = project_dir.to_string_lossy().into_owned();
    let project_id = repo::create_project(&db, NewProject { name: name.into(), path, color_index: None })
        .await
        .unwrap()
        .id;
    Env { db, agents, project_dir, project_id }
}

impl Env {
    async fn start(&self, body: &str) -> i64 {
        let t = NewTicket { project_id: self.project_id, title: "e2e".into(), body: body.into() };
        let id = repo::create_ticket(&self.db, t).await.unwrap().id;
        repo::move_ticket(&self.db, id, TicketStatus::Ready, None).await.unwrap();
        self.agents.kick().await.unwrap();
        id
    }

    async fn wait_until(&self, id: i64, what: &str, done: impl Fn(&Ticket, &[TicketEvent]) -> bool) {
        let start = Instant::now();
        loop {
            let t = repo::get_ticket(&self.db, id).await.unwrap();
            let events = repo::list_events(&self.db, id).await.unwrap();
            if done(&t, &events) {
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(180),
                "timed out waiting for {what}, status {:?}",
                t.status
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn wait_for(&self, id: i64, status: TicketStatus) {
        self.wait_until(id, &format!("{status:?}"), |t, _| t.status == status).await;
    }
}

async fn simple_task_finishes(agent: &str) {
    let e = env("simple", agent).await;
    let id =
        e.start("Create a file hello.txt containing exactly the word hello. Do not ask questions.").await;
    e.wait_for(id, TicketStatus::Done).await;
    assert_eq!(std::fs::read_to_string(e.project_dir.join("hello.txt")).unwrap().trim(), "hello");
    let events = repo::list_events(&e.db, id).await.unwrap();
    assert!(
        events.iter().any(
            |ev| matches!(&ev.data, EventData::ToolUse { name, .. } if name.eq_ignore_ascii_case("write"))
        )
    );
    assert!(events.iter().any(|ev| matches!(ev.data, EventData::TurnResult { is_error: false, .. })));
}

async fn question_and_reply(agent: &str) {
    let e = env("question", agent).await;
    let id = e
        .start(
            "Before doing anything, ask me which programming language I want. \
             Then write a tiny FizzBuzz program in that language into the current folder.",
        )
        .await;
    e.wait_for(id, TicketStatus::NeedsInput).await;
    let events = repo::list_events(&e.db, id).await.unwrap();
    assert!(events.iter().any(|ev| matches!(ev.data, EventData::Question { .. })), "a question was recorded");

    e.agents.reply(id, "Python").await.unwrap();
    e.wait_for(id, TicketStatus::Done).await;
    let files: Vec<_> = std::fs::read_dir(&e.project_dir).unwrap().filter_map(|f| f.ok()).collect();
    assert!(files.iter().any(|f| f.path().extension().is_some_and(|x| x == "py")), "a .py file: {files:?}");
}

async fn cancel_kills_the_agent_and_its_commands(agent: &str) {
    let e = env("cancel", agent).await;
    let marker = format!("wz-e2e-{}", std::process::id());
    let id = e
        .start(&format!(
            "Run this exact shell command in the foreground and wait for it to finish: \
             python3 -c \"import time; time.sleep(120)  # {marker}\""
        ))
        .await;
    // Wait for the process itself: OpenCode only reports a tool call once it has finished.
    e.wait_until(id, "the shell command to run", |_, _| processes_with(&marker) > 0).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let t = e.agents.cancel(id).await.unwrap();
    assert_eq!(t.status, TicketStatus::Planning);
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(processes_with(&marker), 0, "no process of the cancelled run may survive");
}

/// One ignored test per scenario and agent, e.g. `agent::e2e::opencode::question_and_reply`.
macro_rules! per_agent {
    ($($scenario:ident),*) => {
        mod claude {
            $(
                #[tokio::test(flavor = "multi_thread")]
                #[ignore = "runs the real Claude Code CLI"]
                async fn $scenario() { super::$scenario("claude").await }
            )*
        }
        mod opencode {
            $(
                #[tokio::test(flavor = "multi_thread")]
                #[ignore = "runs the real OpenCode CLI"]
                async fn $scenario() { super::$scenario("opencode").await }
            )*
        }
    };
}

per_agent!(simple_task_finishes, question_and_reply, cancel_kills_the_agent_and_its_commands);

/// Live processes whose command line contains `marker` (Linux).
fn processes_with(marker: &str) -> usize {
    let me = std::process::id().to_string();
    std::fs::read_dir("/proc")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()) && n != me))
        .filter(|e| {
            let stat = std::fs::read_to_string(e.path().join("stat")).unwrap_or_default();
            let zombie = stat.rsplit_once(')').is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'));
            let cmd = std::fs::read(e.path().join("cmdline")).unwrap_or_default();
            !zombie && String::from_utf8_lossy(&cmd).contains(marker)
        })
        .count()
}

#[tokio::test]
#[ignore = "needs the real agent CLIs"]
async fn check_reports_the_versions() {
    for agent in ["claude", "opencode"] {
        let version = super::check(agent, None).await.unwrap();
        println!("  {version}");
        assert!(super::check(agent, Some(Path::new("/no/such/agent"))).await.is_err());
    }
}
