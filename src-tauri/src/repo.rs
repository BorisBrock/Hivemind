//! Persistence for projects, tickets and settings.

use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult, invalid};
use crate::models::*;

/// Number of colour slots every palette provides.
pub const PALETTE_SIZE: i64 = 10;

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
        "INSERT INTO tickets (project_id, title, body, status, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(new.project_id)
    .bind(title)
    .bind(&new.body)
    .bind(TicketStatus::Planning)
    .bind(end_of_column(db, TicketStatus::Planning).await?)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?
    .last_insert_rowid();
    get_ticket(db, id).await
}

/// The title can always change; the body (the agent's prompt) only in Planning.
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
    t.updated_at = Utc::now();
    sqlx::query("UPDATE tickets SET title = ?, body = ?, updated_at = ? WHERE id = ?")
        .bind(&t.title)
        .bind(&t.body)
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
    sqlx::query("UPDATE tickets SET status = ?, position = ?, updated_at = ? WHERE id = ?")
        .bind(t.status)
        .bind(t.position)
        .bind(t.updated_at)
        .bind(id)
        .execute(db)
        .await?;
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
        create_ticket(db, NewTicket { project_id, title: title.into(), body: "do it".into() }).await.unwrap()
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
    async fn settings_round_trip_with_defaults() {
        let db = db::open_in_memory().await.unwrap();
        assert_eq!(get_settings(&db).await.unwrap(), Settings::default());
        let s = Settings { theme: "daylight".into(), ..Default::default() };
        save_settings(&db, &s).await.unwrap();
        assert_eq!(get_settings(&db).await.unwrap(), s);
    }
}
