//! IPC commands exposed to the frontend. Thin wrappers over `repo`.

use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::{Manager, State};

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
pub async fn arrange_projects(db: Db<'_>, placements: Vec<ProjectPlacement>) -> AppResult<()> {
    repo::arrange_projects(&db, &placements).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_project(db: Db<'_>, id: i64) -> AppResult<()> {
    repo::delete_project(&db, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_groups(db: Db<'_>) -> AppResult<Vec<ProjectGroup>> {
    repo::list_groups(&db).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_group(db: Db<'_>, name: String) -> AppResult<ProjectGroup> {
    repo::create_group(&db, &name).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_group(db: Db<'_>, id: i64, patch: GroupPatch) -> AppResult<ProjectGroup> {
    repo::update_group(&db, id, patch).await
}

#[tauri::command]
#[specta::specta]
pub async fn reorder_groups(db: Db<'_>, ids: Vec<i64>) -> AppResult<()> {
    repo::reorder_groups(&db, &ids).await
}

/// Shows or hides all projects of the group; returns the changed projects.
#[tauri::command]
#[specta::specta]
pub async fn set_group_hidden(db: Db<'_>, id: i64, hidden: bool) -> AppResult<Vec<Project>> {
    repo::set_group_hidden(&db, id, hidden).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_group(db: Db<'_>, id: i64) -> AppResult<()> {
    repo::delete_group(&db, id).await
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

/// Answer for an agent waiting in Needs Input, or a message to one working In Progress.
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
pub async fn list_trashed_tickets(db: Db<'_>) -> AppResult<Vec<Ticket>> {
    repo::list_trashed_tickets(&db).await
}

/// Permanently deletes every ticket in the trash; returns how many.
#[tauri::command]
#[specta::specta]
pub async fn empty_trash(db: Db<'_>) -> AppResult<u32> {
    Ok(repo::empty_trash(&db).await? as u32)
}

/// Moves a ticket off the board and into the trash.
#[tauri::command]
#[specta::specta]
pub async fn trash_ticket(db: Db<'_>, id: i64) -> AppResult<Ticket> {
    repo::trash_ticket(&db, id).await
}

/// Out of the trash, back to Planning.
#[tauri::command]
#[specta::specta]
pub async fn restore_ticket(db: Db<'_>, id: i64) -> AppResult<Ticket> {
    repo::restore_ticket(&db, id).await
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

/// Opens `path` in the system file manager (Explorer, Finder, or whatever `xdg-open` picks).
#[tauri::command]
#[specta::specta]
pub fn open_folder(path: String) -> AppResult<()> {
    let dir = std::path::Path::new(&path);
    if !dir.is_dir() {
        return Err(crate::error::invalid(format!("Folder not found: {path}")));
    }
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // Not waited for: Explorer reports exit code 1 even on success.
    std::process::Command::new(program)
        .arg(dir)
        .spawn()
        .map_err(|e| crate::error::invalid(format!("Could not open the folder: {e}")))?;
    Ok(())
}

/// GitHub Desktop's launcher, if it is installed. Only Windows for now: the launcher
/// lives at a fixed path there and forwards arguments to the running instance.
fn github_desktop_exe() -> Option<std::path::PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let exe =
        std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("GitHubDesktop/GitHubDesktop.exe");
    exe.exists().then_some(exe)
}

/// Whether GitHub Desktop is installed, so repositories can be opened in it.
#[tauri::command]
#[specta::specta]
pub fn has_github_desktop() -> bool {
    github_desktop_exe().is_some()
}

/// Opens the repository containing `path` in GitHub Desktop, the way its `github open`
/// CLI does. (Its `x-github-client://openLocalRepo` URL is no longer supported.)
/// Desktop offers to add the repository if it doesn't know it yet.
#[tauri::command]
#[specta::specta]
pub fn open_in_github_desktop(path: String) -> AppResult<()> {
    let exe = github_desktop_exe().ok_or_else(|| crate::error::invalid("GitHub Desktop is not installed"))?;
    let root = crate::git::repo_root(std::path::Path::new(&path))
        .ok_or_else(|| crate::error::invalid(format!("Not a git repository: {path}")))?;
    let mut arg = std::ffi::OsString::from("--cli-open=");
    arg.push(root);
    std::process::Command::new(exe)
        .arg(arg)
        .spawn()
        .map_err(|e| crate::error::invalid(format!("Could not open GitHub Desktop: {e}")))?;
    Ok(())
}

/// The existing files among the paths a message mentions, resolved against `cwd`. Their images
/// become loadable for previews.
#[tauri::command]
#[specta::specta]
pub async fn resolve_files(
    app: tauri::AppHandle,
    cwd: Option<String>,
    mentions: Vec<String>,
) -> AppResult<Vec<crate::files::LocalFile>> {
    let cwd = cwd.map(std::path::PathBuf::from);
    let home = app.path().home_dir().ok();
    let files: Vec<_> =
        mentions.iter().filter_map(|m| crate::files::resolve(m, cwd.as_deref(), home.as_deref())).collect();
    for file in files.iter().filter(|f| f.is_image) {
        app.asset_protocol_scope()
            .allow_file(&file.path)
            .map_err(|e| crate::error::invalid(format!("Could not preview {}: {e}", file.path)))?;
    }
    Ok(files)
}

/// Opens a file with its default app if its type is safe, else shows it in the file manager.
#[tauri::command]
#[specta::specta]
pub async fn open_file(path: String) -> AppResult<()> {
    crate::files::open(&path)
}

/// Shows a file in the file manager.
#[tauri::command]
#[specta::specta]
pub async fn reveal_file(path: String) -> AppResult<()> {
    crate::files::reveal(&path)
}

/// Opens a web link in the system browser or mail app.
#[tauri::command]
#[specta::specta]
pub fn open_web_link(url: String) -> AppResult<()> {
    if !crate::files::is_web_url(&url) {
        return Err(crate::error::invalid(format!("Not a web link: {url}")));
    }
    tauri_plugin_opener::open_url(&url, None::<&str>)
        .map_err(|e| crate::error::invalid(format!("Could not open {url}: {e}")))
}

/// Saves a pasted image (base64) in the attachment folder; returns its absolute path.
#[tauri::command]
#[specta::specta]
pub fn save_attachment(
    attachments: State<'_, crate::attachments::Attachments>,
    data_base64: String,
    extension: String,
) -> AppResult<String> {
    crate::attachments::save(&attachments.0, &data_base64, &extension)
}
