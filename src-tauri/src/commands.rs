//! IPC commands exposed to the frontend. Thin wrappers over `repo`.

use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::State;

use crate::agent::manager::AgentManager;
use crate::error::AppResult;
use crate::models::*;
use crate::repo;

type Db<'a> = State<'a, SqlitePool>;
type Agents<'a> = State<'a, Arc<AgentManager>>;

#[tauri::command]
#[specta::specta]
pub async fn list_projects(db: Db<'_>) -> AppResult<Vec<Project>> {
    repo::list_projects(&db).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_project(db: Db<'_>, project: NewProject) -> AppResult<Project> {
    repo::create_project(&db, project).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_project(db: Db<'_>, id: i64, patch: ProjectPatch) -> AppResult<Project> {
    repo::update_project(&db, id, patch).await
}

#[tauri::command]
#[specta::specta]
pub async fn reorder_projects(db: Db<'_>, ids: Vec<i64>) -> AppResult<()> {
    repo::reorder_projects(&db, &ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_project(db: Db<'_>, id: i64) -> AppResult<()> {
    repo::delete_project(&db, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_tickets(db: Db<'_>) -> AppResult<Vec<Ticket>> {
    repo::list_tickets(&db).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_ticket(db: Db<'_>, ticket: NewTicket) -> AppResult<Ticket> {
    repo::create_ticket(&db, ticket).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_ticket(db: Db<'_>, id: i64, patch: TicketPatch) -> AppResult<Ticket> {
    repo::update_ticket(&db, id, patch).await
}

#[tauri::command]
#[specta::specta]
pub async fn move_ticket(
    db: Db<'_>,
    agents: Agents<'_>,
    id: i64,
    to: TicketStatus,
    before_id: Option<i64>,
) -> AppResult<Ticket> {
    let ticket = repo::move_ticket(&db, id, to, before_id).await?;
    if to == TicketStatus::Ready {
        agents.kick().await?;
        return repo::get_ticket(&db, id).await; // may already be In Progress
    }
    Ok(ticket)
}

#[tauri::command]
#[specta::specta]
pub async fn list_ticket_events(db: Db<'_>, ticket_id: i64) -> AppResult<Vec<TicketEvent>> {
    repo::list_events(&db, ticket_id).await
}

/// Answer for an agent waiting in Needs Input.
#[tauri::command]
#[specta::specta]
pub async fn reply_to_ticket(agents: Agents<'_>, id: i64, text: String) -> AppResult<Ticket> {
    agents.reply(id, &text).await
}

/// Done → Ready; `comment` becomes the next prompt of the same agent session.
#[tauri::command]
#[specta::specta]
pub async fn send_back_to_ready(agents: Agents<'_>, id: i64, comment: String) -> AppResult<Ticket> {
    agents.send_back(id, &comment).await
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_ticket(agents: Agents<'_>, id: i64) -> AppResult<Ticket> {
    agents.cancel(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_ticket(db: Db<'_>, id: i64) -> AppResult<()> {
    repo::delete_ticket(&db, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_usage(db: Db<'_>) -> AppResult<Usage> {
    repo::get_usage(&db).await
}

/// Checks that the agent CLI can be found and runs; returns its version.
#[tauri::command]
#[specta::specta]
pub async fn check_agent(agent: String, path: String) -> AppResult<String> {
    let path = Some(path.trim()).filter(|p| !p.is_empty()).map(std::path::Path::new);
    crate::agent::check(&agent, path).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(db: Db<'_>) -> AppResult<Settings> {
    repo::get_settings(&db).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_settings(db: Db<'_>, settings: Settings) -> AppResult<()> {
    repo::save_settings(&db, &settings).await
}

/// Checked-out git branch of `path`, or `None` if it isn't in a git repository.
#[tauri::command]
#[specta::specta]
pub fn git_branch(path: String) -> Option<String> {
    crate::git::current_branch(std::path::Path::new(&path))
}
