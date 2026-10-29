//! IPC commands exposed to the frontend. Thin wrappers over `repo`.

use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::models::*;
use crate::repo;

type Db<'a> = State<'a, SqlitePool>;

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
pub async fn move_ticket(db: Db<'_>, id: i64, to: TicketStatus, index: u32) -> AppResult<Ticket> {
    repo::move_ticket(&db, id, to, index as usize).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_ticket(db: Db<'_>, id: i64) -> AppResult<()> {
    repo::delete_ticket(&db, id).await
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
