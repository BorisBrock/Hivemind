//! Owns every agent-driven ticket transition: starting Ready tickets (one sequential ticket
//! per project, parallel ones right away), following a run to its end, user replies and
//! messages to working agents, sending back, cancelling and crash recovery.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::{mpsc, oneshot};

use super::{AgentBackend, AgentEvent, RunRequest};
use crate::error::{AppError, AppResult, invalid};
use crate::models::{AgentOptions, EventData, Ticket, TicketEvent, TicketStatus, Usage};
use crate::repo;

/// Tells the UI about changes the agent manager makes.
pub trait Notifier: Send + Sync {
    fn ticket_changed(&self, ticket: &Ticket);
    fn event_added(&self, event: &TicketEvent);
    fn usage_changed(&self, usage: &Usage);
}

pub struct AgentManager {
    db: SqlitePool,
    backend: Arc<dyn AgentBackend>,
    notifier: Arc<dyn Notifier>,
    /// Running agents, by ticket id. A run that is no longer in here when it ends was
    /// cancelled, and its ticket is no longer its business.
    runs: Mutex<HashMap<i64, Active>>,
    /// Serialises scheduling so a project never gets two sequential runs.
    schedule: tokio::sync::Mutex<()>,
}

/// A running agent.
struct Active {
    cancel: oneshot::Sender<oneshot::Sender<()>>,
    /// Delivers messages mid-turn, for backends that can take them.
    messages: Option<mpsc::UnboundedSender<String>>,
    /// Messages the agent gets as its next turn, once the current one ends.
    queued: Vec<String>,
}

impl AgentManager {
    pub fn new(db: SqlitePool, backend: Arc<dyn AgentBackend>, notifier: Arc<dyn Notifier>) -> Arc<Self> {
        Arc::new(Self { db, backend, notifier, runs: Mutex::default(), schedule: Default::default() })
    }

    /// Starts every Ready ticket that may run: all parallel ones, and the top sequential one
    /// of each project whose sequential slot is free.
    // Boxed with an explicit `Send`: a finished run kicks the scheduler again, and that
    // recursion is more than the compiler can prove `Send` on its own.
    pub fn kick(self: &Arc<Self>) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>> {
        Box::pin(async move {
            let _guard = self.schedule.lock().await;
            for project_id in repo::projects_with_startable(&self.db).await? {
                while let Some(plan) = repo::claim_next_ready(&self.db, project_id).await? {
                    self.notifier.ticket_changed(&plan.ticket);
                    self.start(plan.ticket, plan.cwd.into(), plan.prompt, plan.resume).await?;
                }
            }
            Ok(())
        })
    }

    /// Answers an agent that is waiting in Needs Input, continuing the same session, or
    /// passes a message to an agent that is still working on an In Progress ticket.
    pub async fn reply(self: &Arc<Self>, ticket_id: i64, text: &str) -> AppResult<Ticket> {
        let text = text.trim();
        if text.is_empty() {
            return Err(invalid("the reply must not be empty"));
        }
        let ticket = repo::get_ticket(&self.db, ticket_id).await?;
        if ticket.status == TicketStatus::InProgress {
            return self.interject(ticket, text).await;
        }
        if ticket.status != TicketStatus::NeedsInput || self.is_running(ticket_id) {
            return Err(AppError::InvalidTransition { from: ticket.status, to: TicketStatus::InProgress });
        }
        let cwd = repo::get_project(&self.db, ticket.project_id).await?.path;
        let ticket = self.set_status(ticket_id, TicketStatus::InProgress).await?;
        let (prompt, resume) = continuation(&ticket, text);
        self.start(ticket.clone(), cwd.into(), prompt, resume).await?;
        Ok(ticket)
    }

    /// Passes a message to the agent of an In Progress ticket: right away if its backend
    /// takes messages mid-turn, else as its next turn.
    async fn interject(&self, ticket: Ticket, text: &str) -> AppResult<Ticket> {
        {
            let mut runs = self.runs.lock().unwrap();
            let active = runs.get_mut(&ticket.id).ok_or_else(|| invalid("the agent is not running"))?;
            let sent = active.messages.as_ref().is_some_and(|m| m.send(text.to_string()).is_ok());
            if !sent {
                active.queued.push(text.to_string());
            }
        }
        self.record(ticket.id, EventData::UserPrompt { text: text.to_string() }).await?;
        Ok(ticket)
    }

    /// Done → Ready with a comment; the scheduler continues the session with it.
    pub async fn send_back(self: &Arc<Self>, ticket_id: i64, comment: &str) -> AppResult<Ticket> {
        let ticket = repo::send_back_to_ready(&self.db, ticket_id, comment).await?;
        self.notifier.ticket_changed(&ticket);
        self.kick().await?;
        repo::get_ticket(&self.db, ticket_id).await
    }

    /// Stops the agent of an active ticket and moves it back to Planning.
    pub async fn cancel(self: &Arc<Self>, ticket_id: i64) -> AppResult<Ticket> {
        let ticket = repo::get_ticket(&self.db, ticket_id).await?;
        if !ticket.status.is_active() {
            return Err(AppError::InvalidTransition { from: ticket.status, to: TicketStatus::Planning });
        }
        let active = self.runs.lock().unwrap().remove(&ticket_id);
        if let Some(Active { cancel, .. }) = active {
            let (ack_tx, ack_rx) = oneshot::channel();
            if cancel.send(ack_tx).is_ok() {
                let _ = ack_rx.await;
            }
        }
        let ticket = repo::reset_to_planning(&self.db, ticket_id).await?;
        self.record(
            ticket_id,
            EventData::System { message: "Cancelled. The next run starts a new session.".into() },
        )
        .await?;
        self.notifier.ticket_changed(&ticket);
        self.kick().await?;
        Ok(ticket)
    }

    /// At start-up: tickets left In Progress lost their agent; let the user continue them.
    pub async fn recover(&self) -> AppResult<()> {
        for id in repo::mark_interrupted(&self.db).await? {
            let message = "Interrupted: Hivemind was closed while the agent was working. \
                           Reply to let it continue, or cancel the ticket."
                .into();
            self.record(id, EventData::System { message }).await?;
        }
        Ok(())
    }

    /// On app exit: stop all agents without touching ticket states (`recover` handles them).
    pub async fn shutdown(&self) {
        let runs: Vec<_> = self.runs.lock().unwrap().drain().collect();
        let mut acks = Vec::new();
        for (_, Active { cancel, .. }) in runs {
            let (tx, rx) = oneshot::channel();
            if cancel.send(tx).is_ok() {
                acks.push(rx);
            }
        }
        let _ = tokio::time::timeout(Duration::from_secs(5), futures_all(acks)).await;
    }

    #[cfg(test)]
    pub fn running_count(&self) -> usize {
        self.runs.lock().unwrap().len()
    }

    fn is_running(&self, ticket_id: i64) -> bool {
        self.runs.lock().unwrap().contains_key(&ticket_id)
    }

    /// Starts an agent run for a ticket that is already In Progress.
    async fn start(
        self: &Arc<Self>,
        ticket: Ticket,
        cwd: PathBuf,
        prompt: String,
        resume: Option<String>,
    ) -> AppResult<()> {
        if resume.is_none() {
            self.record(ticket.id, EventData::System { message: "Started a new agent session.".into() })
                .await?;
        }
        self.record(ticket.id, EventData::UserPrompt { text: prompt.clone() }).await?;
        self.launch(ticket, cwd, prompt, resume).await
    }

    /// Starts the agent itself; the prompt is already in the transcript.
    // Boxed like `kick`: a run that ends with queued messages launches the next one.
    fn launch(
        self: &Arc<Self>,
        ticket: Ticket,
        cwd: PathBuf,
        prompt: String,
        resume: Option<String>,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>> {
        Box::pin(async move {
            let settings = repo::get_settings(&self.db).await?;
            // A session can only be continued by the agent that started it.
            let agent = match (&resume, &ticket.agent) {
                (Some(_), Some(agent)) => agent.clone(),
                _ => settings.agent.clone(),
            };
            let options = AgentOptions::for_agent(&settings, &agent);
            match self.backend.start(RunRequest { cwd, prompt, resume, options }) {
                Ok(run) => {
                    let active = Active { cancel: run.cancel, messages: run.messages, queued: vec![] };
                    self.runs.lock().unwrap().insert(ticket.id, active);
                    tokio::spawn(self.clone().follow(ticket.id, agent, run.events));
                }
                Err(e) => {
                    self.record(ticket.id, EventData::Error { message: e.to_string() }).await?;
                    self.set_status(ticket.id, TicketStatus::Done).await?;
                }
            }
            Ok(())
        })
    }

    /// Records a run's events and moves the ticket on when the run ends.
    async fn follow(self: Arc<Self>, ticket_id: i64, agent: String, mut events: mpsc::Receiver<AgentEvent>) {
        if let Err(e) = self.follow_inner(ticket_id, &agent, &mut events).await {
            // Database trouble: make sure the ticket doesn't stay "running" forever.
            let _ = self.record(ticket_id, EventData::Error { message: e.to_string() }).await;
            if self.runs.lock().unwrap().remove(&ticket_id).is_some() {
                let _ = self.set_status(ticket_id, TicketStatus::NeedsInput).await;
            }
        }
    }

    async fn follow_inner(
        self: &Arc<Self>,
        ticket_id: i64,
        agent: &str,
        events: &mut mpsc::Receiver<AgentEvent>,
    ) -> AppResult<()> {
        let mut asked = false;
        let mut blocked: Option<Option<String>> = None;
        let mut failed = false;
        let mut exit_error = None;
        let mut undelivered = Vec::new();
        while let Some(event) = events.recv().await {
            match event {
                AgentEvent::Session(id) => repo::set_session(&self.db, ticket_id, &id, agent).await?,
                AgentEvent::Transcript(data) => {
                    asked |= matches!(data, EventData::Question { .. });
                    failed |= matches!(data, EventData::TurnResult { is_error: true, .. });
                    if let EventData::TurnResult { input_tokens, output_tokens, cost_usd, .. } = data {
                        repo::log_turn(&self.db, ticket_id, input_tokens, output_tokens, cost_usd).await?;
                        self.notifier.usage_changed(&repo::get_usage(&self.db).await?);
                    }
                    self.record(ticket_id, data).await?;
                }
                AgentEvent::Limits(limits) => {
                    repo::save_limits(&self.db, &limits).await?;
                    self.notifier.usage_changed(&repo::get_usage(&self.db).await?);
                }
                AgentEvent::Blocked { needs_action } => blocked = Some(needs_action),
                AgentEvent::Undelivered(text) => undelivered.push(text),
                AgentEvent::Exited { error } => exit_error = error,
            }
        }
        let Some(active) = self.runs.lock().unwrap().remove(&ticket_id) else {
            return Ok(()); // cancelled or shut down
        };
        undelivered.extend(active.queued);
        if !undelivered.is_empty() {
            if exit_error.is_none() && !failed {
                // Messages sent while the agent worked: they are its next turn.
                let ticket = repo::get_ticket(&self.db, ticket_id).await?;
                let cwd = repo::get_project(&self.db, ticket.project_id).await?.path;
                let (prompt, resume) = continuation(&ticket, &undelivered.join("\n\n"));
                return self.launch(ticket, cwd.into(), prompt, resume).await;
            }
            let message = "The agent stopped before it could read your message. \
                           Send the ticket back to repeat it."
                .into();
            self.record(ticket_id, EventData::System { message }).await?;
        }
        let status = if let Some(message) = exit_error {
            self.record(ticket_id, EventData::Error { message }).await?;
            TicketStatus::Done
        } else if failed {
            TicketStatus::Done
        } else if asked {
            TicketStatus::NeedsInput
        } else if let Some(needs_action) = blocked {
            // Asked in plain text instead of using the tool.
            if let Some(question) = needs_action {
                self.record(ticket_id, EventData::Question { question, options: vec![] }).await?;
            }
            TicketStatus::NeedsInput
        } else {
            TicketStatus::Done
        };
        let ticket = self.set_status(ticket_id, status).await?;
        if status == TicketStatus::Done && !ticket.parallel {
            self.kick().await?; // the project is free for its next ticket
        }
        Ok(())
    }

    async fn set_status(&self, ticket_id: i64, status: TicketStatus) -> AppResult<Ticket> {
        let ticket = repo::set_status(&self.db, ticket_id, status).await?;
        self.notifier.ticket_changed(&ticket);
        Ok(ticket)
    }

    async fn record(&self, ticket_id: i64, data: EventData) -> AppResult<()> {
        let event = repo::append_event(&self.db, ticket_id, data).await?;
        self.notifier.event_added(&event);
        Ok(())
    }
}

/// Prompt and session for continuing a ticket with `text`.
fn continuation(ticket: &Ticket, text: &str) -> (String, Option<String>) {
    match &ticket.agent_session_id {
        Some(session) => (text.to_string(), Some(session.clone())),
        // The previous run never got a session: start over with the full task.
        None => (format!("{}\n\n{text}", ticket.body), None),
    }
}

async fn futures_all(receivers: Vec<oneshot::Receiver<()>>) {
    for rx in receivers {
        let _ = rx.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Run;
    use crate::db;
    use crate::models::{NewProject, NewTicket};

    /// A backend whose runs the test drives by hand.
    #[derive(Default)]
    struct FakeBackend {
        runs: Mutex<Vec<FakeRun>>,
    }

    struct FakeRun {
        request: RunRequest,
        events: mpsc::Sender<AgentEvent>,
        cancel: Option<oneshot::Receiver<oneshot::Sender<()>>>,
        messages: mpsc::UnboundedReceiver<String>,
    }

    impl AgentBackend for FakeBackend {
        fn start(&self, request: RunRequest) -> AppResult<Run> {
            if request.prompt == "fail to start" {
                return Err(invalid("no agent installed"));
            }
            let (tx, rx) = mpsc::channel(16);
            let (cancel_tx, cancel_rx) = oneshot::channel();
            let (messages_tx, messages_rx) = mpsc::unbounded_channel();
            // Like the real backends: only Claude takes messages mid-turn.
            let messages = (request.options.agent == "claude").then_some(messages_tx);
            self.runs.lock().unwrap().push(FakeRun {
                request,
                events: tx,
                cancel: Some(cancel_rx),
                messages: messages_rx,
            });
            Ok(Run { events: rx, cancel: cancel_tx, messages })
        }
    }

    impl FakeBackend {
        fn requests(&self) -> Vec<RunRequest> {
            self.runs.lock().unwrap().iter().map(|r| r.request.clone()).collect()
        }

        /// Plays a whole turn on run `i` and ends it.
        async fn finish(&self, i: usize, events: Vec<AgentEvent>) {
            let tx = self.runs.lock().unwrap()[i].events.clone();
            for e in events {
                tx.send(e).await.unwrap();
            }
            tx.send(AgentEvent::Exited { error: None }).await.unwrap();
            self.runs.lock().unwrap()[i].events = mpsc::channel(1).0; // drop the sender: run over
        }
    }

    struct NoNotifier;
    impl Notifier for NoNotifier {
        fn ticket_changed(&self, _: &Ticket) {}
        fn event_added(&self, _: &TicketEvent) {}
        fn usage_changed(&self, _: &Usage) {}
    }

    struct Env {
        db: SqlitePool,
        fake: Arc<FakeBackend>,
        manager: Arc<AgentManager>,
    }

    async fn env() -> Env {
        let db = db::open_in_memory().await.unwrap();
        let fake = Arc::new(FakeBackend::default());
        let manager = AgentManager::new(db.clone(), fake.clone(), Arc::new(NoNotifier));
        Env { db, fake, manager }
    }

    impl Env {
        async fn project(&self) -> i64 {
            let path = std::env::temp_dir().to_string_lossy().into_owned();
            repo::create_project(
                &self.db,
                NewProject { name: "p".into(), path, color_index: None, group_id: None },
            )
            .await
            .unwrap()
            .id
        }

        async fn ready(&self, project_id: i64, body: &str) -> i64 {
            self.ready_with(project_id, body, false).await
        }

        async fn ready_parallel(&self, project_id: i64, body: &str) -> i64 {
            self.ready_with(project_id, body, true).await
        }

        async fn ready_with(&self, project_id: i64, body: &str, parallel: bool) -> i64 {
            let t = NewTicket { project_id, title: body.into(), body: body.into(), parallel };
            let id = repo::create_ticket(&self.db, t).await.unwrap().id;
            repo::move_ticket(&self.db, id, TicketStatus::Ready, None).await.unwrap();
            self.manager.kick().await.unwrap();
            id
        }

        async fn status(&self, id: i64) -> TicketStatus {
            repo::get_ticket(&self.db, id).await.unwrap().status
        }

        /// Waits for the background follower to process what the test sent.
        async fn wait_for(&self, id: i64, status: TicketStatus) {
            for _ in 0..200 {
                if self.status(id).await == status {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            panic!("ticket {id} never reached {status:?}, is {:?}", self.status(id).await);
        }
    }

    fn text(t: &str) -> AgentEvent {
        AgentEvent::Transcript(EventData::AssistantText { text: t.into() })
    }

    #[tokio::test]
    async fn runs_one_ticket_per_project_in_order() {
        let e = env().await;
        let (p1, p2) = (e.project().await, e.project().await);
        let a = e.ready(p1, "a").await;
        let b = e.ready(p1, "b").await;
        let c = e.ready(p2, "c").await;
        assert_eq!(
            (e.status(a).await, e.status(b).await, e.status(c).await),
            (TicketStatus::InProgress, TicketStatus::Ready, TicketStatus::InProgress)
        );
        assert_eq!(e.manager.running_count(), 2);

        e.fake.finish(0, vec![AgentEvent::Session("s-a".into()), text("done")]).await;
        e.wait_for(a, TicketStatus::Done).await;
        e.wait_for(b, TicketStatus::InProgress).await;
        let prompts: Vec<_> = e.fake.requests().into_iter().map(|r| (r.prompt, r.resume)).collect();
        assert_eq!(prompts, [("a".into(), None), ("c".into(), None), ("b".into(), None)]);
        assert_eq!(repo::get_ticket(&e.db, a).await.unwrap().agent_session_id.as_deref(), Some("s-a"));
    }

    #[tokio::test]
    async fn parallel_tickets_run_alongside_the_queue() {
        let e = env().await;
        let p = e.project().await;
        let a = e.ready(p, "a").await;
        let q1 = e.ready_parallel(p, "q1").await;
        let b = e.ready(p, "b").await;
        let q2 = e.ready_parallel(p, "q2").await;
        assert_eq!(
            (e.status(a).await, e.status(q1).await, e.status(b).await, e.status(q2).await),
            (
                TicketStatus::InProgress,
                TicketStatus::InProgress,
                TicketStatus::Ready,
                TicketStatus::InProgress
            )
        );

        // A parallel ticket ending doesn't free the sequential slot…
        e.fake.finish(1, vec![text("answer")]).await;
        e.wait_for(q1, TicketStatus::Done).await;
        assert_eq!(e.status(b).await, TicketStatus::Ready);
        // …and an active parallel ticket doesn't hold up the queue.
        e.fake.finish(0, vec![text("done")]).await;
        e.wait_for(a, TicketStatus::Done).await;
        e.wait_for(b, TicketStatus::InProgress).await;
        assert_eq!(e.status(q2).await, TicketStatus::InProgress);
    }

    #[tokio::test]
    async fn questions_wait_for_a_reply_that_resumes_the_session() {
        let e = env().await;
        let p = e.project().await;
        let t = e.ready(p, "task").await;
        let question = EventData::Question { question: "A or B?".into(), options: vec![] };
        e.fake.finish(0, vec![AgentEvent::Session("s-1".into()), AgentEvent::Transcript(question)]).await;
        e.wait_for(t, TicketStatus::NeedsInput).await;

        // The project stays busy while the agent waits.
        let other = e.ready(p, "other").await;
        assert_eq!(e.status(other).await, TicketStatus::Ready);

        e.manager.reply(t, "B").await.unwrap();
        assert_eq!(e.status(t).await, TicketStatus::InProgress);
        let last = e.fake.requests().pop().unwrap();
        assert_eq!((last.prompt.as_str(), last.resume.as_deref()), ("B", Some("s-1")));
    }

    #[tokio::test]
    async fn sessions_continue_with_the_agent_that_started_them() {
        let e = env().await;
        let p = e.project().await;
        let t = e.ready(p, "task").await;
        let question = EventData::Question { question: "?".into(), options: vec![] };
        e.fake.finish(0, vec![AgentEvent::Session("s-1".into()), AgentEvent::Transcript(question)]).await;
        e.wait_for(t, TicketStatus::NeedsInput).await;

        let mut settings = repo::get_settings(&e.db).await.unwrap();
        settings.agent = "opencode".into();
        settings.opencode_model = "opencode/big-pickle".into();
        repo::save_settings(&e.db, &settings).await.unwrap();

        e.manager.reply(t, "go on").await.unwrap();
        let other = e.ready(e.project().await, "new").await;
        assert_eq!(e.status(other).await, TicketStatus::InProgress);
        let agents: Vec<_> =
            e.fake.requests().into_iter().map(|r| (r.options.agent, r.options.model)).collect();
        assert_eq!(
            agents,
            [
                ("claude".into(), None),
                ("claude".into(), None),
                ("opencode".into(), Some("opencode/big-pickle".into()))
            ]
        );
    }

    #[tokio::test]
    async fn messages_reach_a_working_agent_mid_turn() {
        let e = env().await;
        let p = e.project().await;
        let t = e.ready(p, "task").await;
        e.manager.reply(t, " what are you doing? ").await.unwrap();
        let received = e.fake.runs.lock().unwrap()[0].messages.try_recv().unwrap();
        assert_eq!(received, "what are you doing?");
        let events = repo::list_events(&e.db, t).await.unwrap();
        assert!(
            matches!(&events.last().unwrap().data, EventData::UserPrompt { text } if text == "what are you doing?")
        );

        e.fake.finish(0, vec![text("done")]).await;
        e.wait_for(t, TicketStatus::Done).await;
        assert_eq!(e.fake.requests().len(), 1);
        assert!(e.manager.reply(t, "too late").await.is_err());
    }

    #[tokio::test]
    async fn messages_that_miss_the_turn_become_the_next_one() {
        let e = env().await;
        let p = e.project().await;
        let mut settings = repo::get_settings(&e.db).await.unwrap();
        settings.agent = "opencode".into();
        repo::save_settings(&e.db, &settings).await.unwrap();
        let t = e.ready(p, "task").await;
        e.manager.reply(t, "one").await.unwrap();
        e.manager.reply(t, "two").await.unwrap();
        // A message the backend accepted but could no longer pass on.
        let undelivered = AgentEvent::Undelivered("three".into());
        e.fake.finish(0, vec![AgentEvent::Session("s-1".into()), undelivered]).await;

        for _ in 0..200 {
            if e.fake.requests().len() == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let last = e.fake.requests().pop().unwrap();
        assert_eq!((last.prompt.as_str(), last.resume.as_deref()), ("three\n\none\n\ntwo", Some("s-1")));
        assert_eq!(e.status(t).await, TicketStatus::InProgress);
        let prompts = repo::list_events(&e.db, t)
            .await
            .unwrap()
            .into_iter()
            .filter(|ev| matches!(ev.data, EventData::UserPrompt { .. }))
            .count();
        assert_eq!(prompts, 3, "the queued messages are not recorded twice");

        e.fake.finish(1, vec![text("done")]).await;
        e.wait_for(t, TicketStatus::Done).await;
    }

    #[tokio::test]
    async fn messages_are_dropped_with_a_note_when_the_run_fails() {
        let e = env().await;
        let p = e.project().await;
        let t = e.ready(p, "task").await;
        let tx = e.fake.runs.lock().unwrap()[0].events.clone();
        tx.send(AgentEvent::Undelivered("hello?".into())).await.unwrap();
        tx.send(AgentEvent::Exited { error: Some("crashed".into()) }).await.unwrap();
        drop(tx);
        e.fake.runs.lock().unwrap()[0].events = mpsc::channel(1).0;
        e.wait_for(t, TicketStatus::Done).await;
        assert_eq!(e.fake.requests().len(), 1);
        let events = repo::list_events(&e.db, t).await.unwrap();
        assert!(events.iter().any(
            |ev| matches!(&ev.data, EventData::System { message } if message.contains("before it could read"))
        ));
    }

    #[tokio::test]
    async fn plain_text_questions_are_caught_by_the_turn_summary() {
        let e = env().await;
        let p = e.project().await;
        let t = e.ready(p, "task").await;
        e.fake.finish(0, vec![AgentEvent::Blocked { needs_action: Some("pick a name".into()) }]).await;
        e.wait_for(t, TicketStatus::NeedsInput).await;
        let events = repo::list_events(&e.db, t).await.unwrap();
        assert!(
            matches!(&events.last().unwrap().data, EventData::Question { question, .. } if question == "pick a name")
        );
    }

    #[tokio::test]
    async fn cancel_stops_the_agent_and_frees_the_project() {
        let e = env().await;
        let p = e.project().await;
        let t = e.ready(p, "task").await;
        let next = e.ready(p, "next").await;

        let cancel_rx = e.fake.runs.lock().unwrap()[0].cancel.take().unwrap();
        let backend_side = tokio::spawn(async move {
            let ack = cancel_rx.await.unwrap();
            ack.send(()).unwrap();
        });
        let ticket = e.manager.cancel(t).await.unwrap();
        backend_side.await.unwrap();
        assert_eq!((ticket.status, ticket.agent_session_id), (TicketStatus::Planning, None));
        assert_eq!(e.status(next).await, TicketStatus::InProgress);

        // The cancelled run ending later must not move the ticket.
        e.fake.finish(0, vec![text("late")]).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(e.status(t).await, TicketStatus::Planning);
    }

    #[tokio::test]
    async fn errors_end_in_done_and_send_back_continues() {
        let e = env().await;
        let p = e.project().await;
        let t = e.ready(p, "task").await;
        let tx = e.fake.runs.lock().unwrap()[0].events.clone();
        tx.send(AgentEvent::Session("s-1".into())).await.unwrap();
        tx.send(AgentEvent::Exited { error: Some("crashed".into()) }).await.unwrap();
        drop(tx);
        e.fake.runs.lock().unwrap()[0].events = mpsc::channel(1).0;
        e.wait_for(t, TicketStatus::Done).await;

        e.manager.send_back(t, "try again").await.unwrap();
        assert_eq!(e.status(t).await, TicketStatus::InProgress);
        let last = e.fake.requests().pop().unwrap();
        assert_eq!((last.prompt.as_str(), last.resume.as_deref()), ("try again", Some("s-1")));
    }

    #[tokio::test]
    async fn failing_to_start_is_reported() {
        let e = env().await;
        let p = e.project().await;
        let t = e.ready(p, "fail to start").await;
        assert_eq!(e.status(t).await, TicketStatus::Done);
        let events = repo::list_events(&e.db, t).await.unwrap();
        assert!(
            matches!(&events.last().unwrap().data, EventData::Error { message } if message.contains("no agent"))
        );
    }
}
