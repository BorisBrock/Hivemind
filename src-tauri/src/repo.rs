//! Persistence for projects, tickets and settings.

use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult, invalid};
use crate::models::*;

/// Number of colour slots every palette provides.
pub const PALETTE_SIZE: i64 = 12;

// ---- projects ---------------------------------------------------------------

pub async fn list_projects(db: &SqlitePool) -> AppResult<Vec<Project>> {
    Ok(sqlx::query_as("SELECT * FROM projects ORDER BY sort_order, id").fetch_all(db).await?)
}

pub async fn get_project(db: &SqlitePool, id: i64) -> AppResult<Project> {
    Ok(sqlx::query_as("SELECT * FROM projects WHERE id = ?").bind(id).fetch_one(db).await?)
}

pub async fn create_project(db: &SqlitePool, new: NewProject) -> AppResult<Project> {
    let name = non_empty(&new.name, "name")?;
    let path = existing_dir(&new.path)?;
    let color_index = match new.color_index {
        Some(c) => check_color(c)?,
        None => next_free_color(db).await?,
    };
    let id = sqlx::query(
        "INSERT INTO projects (name, path, color_index, sort_order, created_at)
         VALUES (?, ?, ?, (SELECT COALESCE(MAX(sort_order), -1) + 1 FROM projects), ?)",
    )
    .bind(name)
    .bind(path)
    .bind(color_index)
    .bind(Utc::now())
    .execute(db)
    .await?
    .last_insert_rowid();
    get_project(db, id).await
}

pub async fn update_project(db: &SqlitePool, id: i64, patch: ProjectPatch) -> AppResult<Project> {
    let mut p = get_project(db, id).await?;
    if let Some(name) = patch.name {
        p.name = non_empty(&name, "name")?.into();
    }
    if let Some(path) = patch.path {
        p.path = existing_dir(&path)?.into();
    }
    if let Some(c) = patch.color_index {
        p.color_index = check_color(c)?;
    }
    if let Some(hidden) = patch.hidden {
        p.hidden = hidden;
    }
    sqlx::query("UPDATE projects SET name = ?, path = ?, color_index = ?, hidden = ? WHERE id = ?")
        .bind(&p.name)
        .bind(&p.path)
        .bind(p.color_index)
        .bind(p.hidden)
        .bind(id)
        .execute(db)
        .await?;
    Ok(p)
}

/// Sets the sidebar order to the order of `ids`.
pub async fn reorder_projects(db: &SqlitePool, ids: &[i64]) -> AppResult<()> {
    let mut tx = db.begin().await?;
    for (order, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE projects SET sort_order = ? WHERE id = ?")
            .bind(order as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Deletes the project and, via cascade, all its tickets and transcripts.
pub async fn delete_project(db: &SqlitePool, id: i64) -> AppResult<()> {
    let running: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tickets WHERE project_id = ? AND status IN ('in_progress', 'needs_input')",
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    if running > 0 {
        return Err(invalid("cancel the running agent before deleting this project"));
    }
    let n = sqlx::query("DELETE FROM projects WHERE id = ?").bind(id).execute(db).await?;
    if n.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn next_free_color(db: &SqlitePool) -> AppResult<i64> {
    let used: Vec<i64> = sqlx::query_scalar("SELECT color_index FROM projects").fetch_all(db).await?;
    Ok((0..PALETTE_SIZE).find(|c| !used.contains(c)).unwrap_or(used.len() as i64 % PALETTE_SIZE))
}

fn check_color(c: i64) -> AppResult<i64> {
    if (0..PALETTE_SIZE).contains(&c) {
        Ok(c)
    } else {
        Err(invalid(format!("color index must be between 0 and {}", PALETTE_SIZE - 1)))
    }
}

// ---- tickets ----------------------------------------------------------------

pub async fn list_tickets(db: &SqlitePool) -> AppResult<Vec<Ticket>> {
    Ok(sqlx::query_as("SELECT * FROM tickets ORDER BY status, position").fetch_all(db).await?)
}

pub async fn get_ticket(db: &SqlitePool, id: i64) -> AppResult<Ticket> {
    Ok(sqlx::query_as("SELECT * FROM tickets WHERE id = ?").bind(id).fetch_one(db).await?)
}

/// New tickets start in Planning, at the bottom of the column.
pub async fn create_ticket(db: &SqlitePool, new: NewTicket) -> AppResult<Ticket> {
    get_project(db, new.project_id).await?;
    let title = non_empty(&new.title, "title")?;
    let now = Utc::now();
    let id = sqlx::query(
        "INSERT INTO tickets (project_id, title, body, parallel, status, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(new.project_id)
    .bind(title)
    .bind(&new.body)
    .bind(new.parallel)
    .bind(TicketStatus::Planning)
    .bind(end_of_column(db, TicketStatus::Planning).await?)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?
    .last_insert_rowid();
    get_ticket(db, id).await
}

/// The title can always change; the body (the agent's prompt) and `parallel` only in Planning.
pub async fn update_ticket(db: &SqlitePool, id: i64, patch: TicketPatch) -> AppResult<Ticket> {
    let mut t = get_ticket(db, id).await?;
    if let Some(title) = patch.title {
        t.title = non_empty(&title, "title")?.into();
    }
    if let Some(body) = patch.body {
        if t.status != TicketStatus::Planning {
            return Err(AppError::NotEditable);
        }
        t.body = body;
    }
    if let Some(parallel) = patch.parallel {
        if t.status != TicketStatus::Planning && parallel != t.parallel {
            return Err(AppError::NotEditable);
        }
        t.parallel = parallel;
    }
    t.updated_at = Utc::now();
    sqlx::query("UPDATE tickets SET title = ?, body = ?, parallel = ?, updated_at = ? WHERE id = ?")
        .bind(&t.title)
        .bind(&t.body)
        .bind(t.parallel)
        .bind(t.updated_at)
        .bind(id)
        .execute(db)
        .await?;
    Ok(t)
}

/// User-initiated move into the `to` column, directly above `before_id` (or to the bottom).
/// Placing relative to a neighbour keeps the order right when hidden projects share the column.
pub async fn move_ticket(
    db: &SqlitePool,
    id: i64,
    to: TicketStatus,
    before_id: Option<i64>,
) -> AppResult<Ticket> {
    let mut t = get_ticket(db, id).await?;
    if !t.status.user_can_move_to(to) {
        return Err(AppError::InvalidTransition { from: t.status, to });
    }
    let column: Vec<(i64, f64)> =
        sqlx::query_as("SELECT id, position FROM tickets WHERE status = ? AND id != ? ORDER BY position")
            .bind(to)
            .bind(id)
            .fetch_all(db)
            .await?;
    let index = match before_id {
        Some(b) => column.iter().position(|(cid, _)| *cid == b).ok_or(AppError::NotFound)?,
        None => column.len(),
    };
    let position_at = |i: usize| column.get(i).map(|(_, p)| *p);
    t.position = match (index.checked_sub(1).and_then(position_at), position_at(index)) {
        (None, None) => 0.0,
        (None, Some(next)) => next - 1.0,
        (Some(prev), None) => prev + 1.0,
        (Some(prev), Some(next)) => (prev + next) / 2.0,
    };
    t.status = to;
    t.updated_at = Utc::now();
    let from = get_ticket(db, id).await?.status;
    // Only if nothing (e.g. the scheduler) changed the status since we read it.
    let n = sqlx::query(
        "UPDATE tickets SET status = ?, position = ?, updated_at = ? WHERE id = ? AND status = ?",
    )
    .bind(t.status)
    .bind(t.position)
    .bind(t.updated_at)
    .bind(id)
    .bind(from)
    .execute(db)
    .await?;
    if n.rows_affected() == 0 {
        return Err(AppError::InvalidTransition { from, to });
    }
    Ok(t)
}

/// Permanently deletes a ticket and its transcript. Running tickets must be cancelled first.
pub async fn delete_ticket(db: &SqlitePool, id: i64) -> AppResult<()> {
    let t = get_ticket(db, id).await?;
    if matches!(t.status, TicketStatus::InProgress | TicketStatus::NeedsInput) {
        return Err(invalid("cancel the agent before deleting this ticket"));
    }
    sqlx::query("DELETE FROM tickets WHERE id = ?").bind(id).execute(db).await?;
    Ok(())
}

async fn end_of_column(db: &SqlitePool, status: TicketStatus) -> AppResult<f64> {
    let max: Option<f64> = sqlx::query_scalar("SELECT MAX(position) FROM tickets WHERE status = ?")
        .bind(status)
        .fetch_one(db)
        .await?;
    Ok(max.map_or(0.0, |m| m + 1.0))
}

// ---- agent runs -------------------------------------------------------------

/// What the agent should do for a ticket that was just claimed for a run.
#[derive(Debug, Clone)]
pub struct RunPlan {
    pub ticket: Ticket,
    pub cwd: String,
    pub prompt: String,
    /// Session to continue; `None` starts a fresh session with the ticket body.
    pub resume: Option<String>,
}

/// A Ready ticket may start if it is parallel, or if no other sequential ticket of its
/// project is active. Parallel tickets never take the project's sequential slot.
const STARTABLE: &str = "t.status = 'ready' AND (t.parallel OR NOT EXISTS (
    SELECT 1 FROM tickets a WHERE a.project_id = t.project_id AND NOT a.parallel
    AND a.status IN ('in_progress', 'needs_input')))";

/// Projects that have a Ready ticket that may start now.
pub async fn projects_with_startable(db: &SqlitePool) -> AppResult<Vec<i64>> {
    Ok(sqlx::query_scalar(&format!("SELECT DISTINCT project_id FROM tickets t WHERE {STARTABLE}"))
        .fetch_all(db)
        .await?)
}

/// Claims the top Ready ticket of a project that may start: moves it to In Progress and
/// consumes its pending prompt. Returns `None` if there is nothing to start.
pub async fn claim_next_ready(db: &SqlitePool, project_id: i64) -> AppResult<Option<RunPlan>> {
    let mut tx = db.begin().await?;
    let next: Option<Ticket> = sqlx::query_as(&format!(
        "SELECT * FROM tickets t WHERE t.project_id = ? AND {STARTABLE} ORDER BY t.position LIMIT 1"
    ))
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(mut ticket) = next else { return Ok(None) };
    let cwd: String = sqlx::query_scalar("SELECT path FROM projects WHERE id = ?")
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;

    let (prompt, resume) = match (ticket.pending_prompt.take(), &ticket.agent_session_id) {
        (Some(comment), Some(session)) => (comment, Some(session.clone())),
        // No session to continue (the last run never started): the full task plus the comment.
        (Some(comment), None) => (format!("{}\n\n{comment}", ticket.body), None),
        (None, _) => (ticket.body.clone(), None),
    };
    ticket.status = TicketStatus::InProgress;
    ticket.updated_at = Utc::now();
    sqlx::query("UPDATE tickets SET status = ?, pending_prompt = NULL, updated_at = ? WHERE id = ?")
        .bind(ticket.status)
        .bind(ticket.updated_at)
        .bind(ticket.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some(RunPlan { ticket, cwd, prompt, resume }))
}

/// Status change made by the agent manager (not subject to the user's move rules).
pub async fn set_status(db: &SqlitePool, id: i64, status: TicketStatus) -> AppResult<Ticket> {
    sqlx::query("UPDATE tickets SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(Utc::now())
        .bind(id)
        .execute(db)
        .await?;
    get_ticket(db, id).await
}

pub async fn set_session(db: &SqlitePool, id: i64, session_id: &str, agent: &str) -> AppResult<()> {
    sqlx::query("UPDATE tickets SET agent_session_id = ?, agent = ? WHERE id = ?")
        .bind(session_id)
        .bind(agent)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Done → Ready with a comment that becomes the next prompt of the same session.
pub async fn send_back_to_ready(db: &SqlitePool, id: i64, comment: &str) -> AppResult<Ticket> {
    let t = get_ticket(db, id).await?;
    if t.status != TicketStatus::Done {
        return Err(AppError::InvalidTransition { from: t.status, to: TicketStatus::Ready });
    }
    let comment = non_empty(comment, "comment")?;
    sqlx::query(
        "UPDATE tickets SET status = 'ready', pending_prompt = ?, position = ?, updated_at = ? WHERE id = ?",
    )
    .bind(comment)
    .bind(end_of_column(db, TicketStatus::Ready).await?)
    .bind(Utc::now())
    .bind(id)
    .execute(db)
    .await?;
    get_ticket(db, id).await
}

/// Back to Planning after a cancel; the next run starts a fresh session.
pub async fn reset_to_planning(db: &SqlitePool, id: i64) -> AppResult<Ticket> {
    sqlx::query(
        "UPDATE tickets SET status = 'planning', agent_session_id = NULL, agent = NULL, pending_prompt = NULL,
         position = ?, updated_at = ? WHERE id = ?",
    )
    .bind(end_of_column(db, TicketStatus::Planning).await?)
    .bind(Utc::now())
    .bind(id)
    .execute(db)
    .await?;
    get_ticket(db, id).await
}

/// Tickets left In Progress by a previous app run; their agent process is gone.
pub async fn mark_interrupted(db: &SqlitePool) -> AppResult<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "UPDATE tickets SET status = 'needs_input', updated_at = ? WHERE status = 'in_progress' RETURNING id",
    )
    .bind(Utc::now())
    .fetch_all(db)
    .await?)
}

// ---- transcript -------------------------------------------------------------

pub async fn list_events(db: &SqlitePool, ticket_id: i64) -> AppResult<Vec<TicketEvent>> {
    let rows: Vec<(i64, i64, i64, String, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, ticket_id, seq, payload, created_at FROM ticket_events WHERE ticket_id = ? ORDER BY seq",
    )
    .bind(ticket_id)
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|(id, ticket_id, seq, payload, created_at)| {
            Ok(TicketEvent { id, ticket_id, seq, created_at, data: serde_json::from_str(&payload)? })
        })
        .collect()
}

pub async fn append_event(db: &SqlitePool, ticket_id: i64, data: EventData) -> AppResult<TicketEvent> {
    let payload = serde_json::to_string(&data)?;
    let kind = payload_kind(&payload);
    let created_at = Utc::now();
    let (id, seq): (i64, i64) = sqlx::query_as(
        "INSERT INTO ticket_events (ticket_id, seq, kind, payload, created_at)
         VALUES (?, (SELECT COALESCE(MAX(seq), 0) + 1 FROM ticket_events WHERE ticket_id = ?), ?, ?, ?)
         RETURNING id, seq",
    )
    .bind(ticket_id)
    .bind(ticket_id)
    .bind(kind)
    .bind(payload)
    .bind(created_at)
    .fetch_one(db)
    .await?;
    Ok(TicketEvent { id, ticket_id, seq, created_at, data })
}

/// The serde tag of an `EventData` JSON string, stored separately for querying.
fn payload_kind(payload: &str) -> String {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|v| v["kind"].as_str().map(String::from))
        .unwrap_or_default()
}

// ---- usage ------------------------------------------------------------------

const LIMITS_KEY: &str = "usage_limits";

pub async fn log_turn(
    db: &SqlitePool,
    ticket_id: i64,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO agent_turns (ticket_id, created_at, input_tokens, output_tokens, cost_usd) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(ticket_id)
    .bind(Utc::now())
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(cost_usd)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn save_limits(db: &SqlitePool, limits: &UsageLimits) -> AppResult<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value")
        .bind(LIMITS_KEY)
        .bind(serde_json::to_string(limits)?)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn get_usage(db: &SqlitePool) -> AppResult<Usage> {
    let raw: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(LIMITS_KEY)
        .fetch_optional(db)
        .await?;
    let limits = raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
    let since = Utc::now() - chrono::Duration::days(7);
    let (week_input_tokens, week_output_tokens, week_cost_usd): (i64, i64, f64) = sqlx::query_as(
        "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0), COALESCE(SUM(cost_usd), 0.0)
         FROM agent_turns WHERE created_at >= ?",
    )
    .bind(since)
    .fetch_one(db)
    .await?;
    Ok(Usage { limits, week_input_tokens, week_output_tokens, week_cost_usd })
}

// ---- settings ---------------------------------------------------------------

const SETTINGS_KEY: &str = "app";

pub async fn get_settings(db: &SqlitePool) -> AppResult<Settings> {
    let raw: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(SETTINGS_KEY)
        .fetch_optional(db)
        .await?;
    // Unknown or broken settings fall back to defaults rather than blocking start-up.
    Ok(raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default())
}

pub async fn save_settings(db: &SqlitePool, settings: &Settings) -> AppResult<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value")
        .bind(SETTINGS_KEY)
        .bind(serde_json::to_string(settings)?)
        .execute(db)
        .await?;
    Ok(())
}

fn existing_dir(path: &str) -> AppResult<&str> {
    let path = non_empty(path, "path")?;
    if std::path::Path::new(path).is_dir() {
        Ok(path)
    } else {
        Err(invalid(format!("folder does not exist: {path}")))
    }
}

fn non_empty<'a>(value: &'a str, field: &str) -> AppResult<&'a str> {
    let v = value.trim();
    if v.is_empty() { Err(invalid(format!("{field} must not be empty"))) } else { Ok(v) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    async fn project(db: &SqlitePool, name: &str) -> Project {
        let path = std::env::temp_dir().to_string_lossy().into_owned();
        create_project(db, NewProject { name: name.into(), path, color_index: None }).await.unwrap()
    }

    async fn ticket(db: &SqlitePool, project_id: i64, title: &str) -> Ticket {
        create_ticket(
            db,
            NewTicket { project_id, title: title.into(), body: "do it".into(), parallel: false },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn projects_get_distinct_colors_and_order() {
        let db = db::open_in_memory().await.unwrap();
        let a = project(&db, "a").await;
        let b = project(&db, "b").await;
        assert_ne!(a.color_index, b.color_index);
        assert_eq!((a.sort_order, b.sort_order), (0, 1));

        delete_project(&db, a.id).await.unwrap();
        let c = project(&db, "c").await;
        assert_eq!(c.color_index, a.color_index, "freed colour is reused");
    }

    #[tokio::test]
    async fn projects_need_an_existing_folder_and_can_be_reordered() {
        let db = db::open_in_memory().await.unwrap();
        let missing = NewProject { name: "x".into(), path: "/does/not/exist".into(), color_index: None };
        assert!(matches!(create_project(&db, missing).await, Err(AppError::Invalid { .. })));

        let a = project(&db, "a").await;
        let b = project(&db, "b").await;
        reorder_projects(&db, &[b.id, a.id]).await.unwrap();
        let names: Vec<String> = list_projects(&db).await.unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["b", "a"]);
    }

    #[tokio::test]
    async fn deleting_project_cascades_to_tickets() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t = ticket(&db, p.id, "t").await;
        delete_project(&db, p.id).await.unwrap();
        assert!(matches!(get_ticket(&db, t.id).await, Err(AppError::NotFound)));
    }

    #[tokio::test]
    async fn moves_follow_user_rules_and_ordering() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t1 = ticket(&db, p.id, "1").await;
        let t2 = ticket(&db, p.id, "2").await;
        let t3 = ticket(&db, p.id, "3").await;

        move_ticket(&db, t1.id, TicketStatus::Ready, None).await.unwrap();
        move_ticket(&db, t2.id, TicketStatus::Ready, Some(t1.id)).await.unwrap();
        move_ticket(&db, t3.id, TicketStatus::Ready, Some(t1.id)).await.unwrap();
        let ready: Vec<String> = list_tickets(&db)
            .await
            .unwrap()
            .into_iter()
            .filter(|t| t.status == TicketStatus::Ready)
            .map(|t| t.title)
            .collect();
        assert_eq!(ready, ["2", "3", "1"]);

        let err = move_ticket(&db, t1.id, TicketStatus::Done, None).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidTransition { .. }));
    }

    #[tokio::test]
    async fn body_is_locked_outside_planning() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t = ticket(&db, p.id, "t").await;
        let body = TicketPatch { body: Some("new".into()), ..Default::default() };

        update_ticket(&db, t.id, body.clone()).await.unwrap();
        move_ticket(&db, t.id, TicketStatus::Ready, None).await.unwrap();
        assert!(matches!(update_ticket(&db, t.id, body).await, Err(AppError::NotEditable)));
        let title = TicketPatch { title: Some("renamed".into()), ..Default::default() };
        assert_eq!(update_ticket(&db, t.id, title).await.unwrap().title, "renamed");
    }

    #[tokio::test]
    async fn claiming_respects_one_active_ticket_per_project() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t1 = ticket(&db, p.id, "1").await;
        let t2 = ticket(&db, p.id, "2").await;
        move_ticket(&db, t1.id, TicketStatus::Ready, None).await.unwrap();
        move_ticket(&db, t2.id, TicketStatus::Ready, None).await.unwrap();
        assert_eq!(projects_with_startable(&db).await.unwrap(), [p.id]);

        let plan = claim_next_ready(&db, p.id).await.unwrap().unwrap();
        assert_eq!((plan.ticket.id, plan.prompt.as_str(), plan.resume), (t1.id, "do it", None));
        assert!(claim_next_ready(&db, p.id).await.unwrap().is_none(), "project is busy");
        assert!(projects_with_startable(&db).await.unwrap().is_empty());
        // The scheduler took it, so the user can no longer drag it back.
        assert!(move_ticket(&db, t1.id, TicketStatus::Planning, None).await.is_err());

        // Parallel tickets start regardless, and don't block the queue.
        let q = create_ticket(
            &db,
            NewTicket { project_id: p.id, title: "q".into(), body: "?".into(), parallel: true },
        )
        .await
        .unwrap();
        move_ticket(&db, q.id, TicketStatus::Ready, None).await.unwrap();
        assert_eq!(claim_next_ready(&db, p.id).await.unwrap().unwrap().ticket.id, q.id);
        set_status(&db, t1.id, TicketStatus::Done).await.unwrap();
        assert_eq!(claim_next_ready(&db, p.id).await.unwrap().unwrap().ticket.id, t2.id);
    }

    #[tokio::test]
    async fn parallel_flag_is_locked_outside_planning() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t = ticket(&db, p.id, "t").await;
        let patch = |parallel| TicketPatch { parallel: Some(parallel), ..Default::default() };
        assert!(update_ticket(&db, t.id, patch(true)).await.unwrap().parallel);
        move_ticket(&db, t.id, TicketStatus::Ready, None).await.unwrap();
        assert!(matches!(update_ticket(&db, t.id, patch(false)).await, Err(AppError::NotEditable)));
        assert!(update_ticket(&db, t.id, patch(true)).await.is_ok(), "unchanged value is fine");
    }

    #[tokio::test]
    async fn sending_back_resumes_the_session_with_the_comment() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t = ticket(&db, p.id, "t").await;
        move_ticket(&db, t.id, TicketStatus::Ready, None).await.unwrap();
        claim_next_ready(&db, p.id).await.unwrap().unwrap();
        set_session(&db, t.id, "sess-1", "claude").await.unwrap();
        set_status(&db, t.id, TicketStatus::Done).await.unwrap();

        assert!(send_back_to_ready(&db, t.id, "  ").await.is_err(), "comment required");
        send_back_to_ready(&db, t.id, "also add tests").await.unwrap();
        let plan = claim_next_ready(&db, p.id).await.unwrap().unwrap();
        assert_eq!(plan.prompt, "also add tests");
        assert_eq!(plan.resume.as_deref(), Some("sess-1"));

        let reset = reset_to_planning(&db, t.id).await.unwrap();
        assert_eq!((reset.status, reset.agent_session_id), (TicketStatus::Planning, None));
    }

    #[tokio::test]
    async fn events_are_numbered_and_round_trip() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t = ticket(&db, p.id, "t").await;
        let a = append_event(&db, t.id, EventData::UserPrompt { text: "hi".into() }).await.unwrap();
        let q = EventData::Question { question: "A or B?".into(), options: vec!["A".into(), "B".into()] };
        let b = append_event(&db, t.id, q.clone()).await.unwrap();
        assert_eq!((a.seq, b.seq), (1, 2));
        let events = list_events(&db, t.id).await.unwrap();
        assert_eq!(events.last().unwrap().data, q);
    }

    #[tokio::test]
    async fn interrupted_runs_wait_for_the_user() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t = ticket(&db, p.id, "t").await;
        move_ticket(&db, t.id, TicketStatus::Ready, None).await.unwrap();
        claim_next_ready(&db, p.id).await.unwrap().unwrap();
        assert_eq!(mark_interrupted(&db).await.unwrap(), [t.id]);
        assert_eq!(get_ticket(&db, t.id).await.unwrap().status, TicketStatus::NeedsInput);
    }

    #[tokio::test]
    async fn usage_survives_ticket_deletion() {
        let db = db::open_in_memory().await.unwrap();
        let p = project(&db, "p").await;
        let t = ticket(&db, p.id, "t").await;
        log_turn(&db, t.id, 1000, 50, 0.25).await.unwrap();
        log_turn(&db, t.id, 500, 25, 0.5).await.unwrap();
        delete_ticket(&db, t.id).await.unwrap();
        let limits = UsageLimits {
            five_hour: Some(UsageWindow { utilization: 0.4, resets_at: None }),
            ..Default::default()
        };
        save_limits(&db, &limits).await.unwrap();

        let usage = get_usage(&db).await.unwrap();
        assert_eq!(
            (usage.week_input_tokens, usage.week_output_tokens, usage.week_cost_usd),
            (1500, 75, 0.75)
        );
        assert_eq!(usage.limits, limits);
    }

    #[tokio::test]
    async fn settings_round_trip_with_defaults() {
        let db = db::open_in_memory().await.unwrap();
        assert_eq!(get_settings(&db).await.unwrap(), Settings::default());
        let s = Settings { theme: "daylight".into(), ..Default::default() };
        save_settings(&db, &s).await.unwrap();
        assert_eq!(get_settings(&db).await.unwrap(), s);
    }
}
