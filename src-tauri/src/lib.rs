mod agent;
mod attachments;
mod commands;
mod db;
mod error;
mod events;
mod files;
mod git;
mod legacy;
mod models;
mod repo;

use std::sync::Arc;

use tauri::Manager;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands, collect_events};

use agent::claude::ClaudeBackend;
use agent::manager::AgentManager;
use agent::opencode::OpenCodeBackend;

const BINDINGS_PATH: &str = "../src/lib/bindings.ts";

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::list_projects,
            commands::create_project,
            commands::update_project,
            commands::arrange_projects,
            commands::delete_project,
            commands::list_groups,
            commands::create_group,
            commands::update_group,
            commands::reorder_groups,
            commands::set_group_hidden,
            commands::delete_group,
            commands::list_tickets,
            commands::create_ticket,
            commands::update_ticket,
            commands::move_ticket,
            commands::delete_ticket,
            commands::list_trashed_tickets,
            commands::trash_ticket,
            commands::empty_trash,
            commands::restore_ticket,
            commands::list_ticket_events,
            commands::reply_to_ticket,
            commands::send_back_to_ready,
            commands::cancel_ticket,
            commands::get_usage,
            commands::check_agent,
            commands::get_settings,
            commands::save_settings,
            commands::git_branch,
            commands::open_folder,
            commands::has_github_desktop,
            commands::open_in_github_desktop,
            commands::resolve_files,
            commands::open_file,
            commands::reveal_file,
            commands::open_web_link,
            commands::save_attachment,
        ])
        .events(collect_events![events::TicketChanged, events::TicketEventAdded, events::UsageChanged])
        .constant("PALETTE_SIZE", repo::PALETTE_SIZE)
        .error_handling(ErrorHandlingMode::Throw)
        // IDs are SQLite rowids, far below 2^53.
        .dangerously_cast_bigints_to_number()
}

fn export_bindings(builder: &Builder<tauri::Wry>) {
    builder
        .export(specta_typescript::Typescript::default().header("// @ts-nocheck\n"), BINDINGS_PATH)
        .expect("failed to export TypeScript bindings");
}

/// Entry point of `hivemind mcp-ask-user`, which Claude Code starts as an MCP server.
pub fn run_mcp_server_if_requested() -> bool {
    if std::env::args().nth(1).as_deref() == Some(agent::mcp::SUBCOMMAND) {
        agent::mcp::serve();
        return true;
    }
    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();
    #[cfg(debug_assertions)]
    export_bindings(&builder);

    tauri::Builder::default()
        .plugin(legacy::plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // Remembers window size and position between runs.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let pool = tauri::async_runtime::block_on(db::open(&dir.join(legacy::DB)))?;
            let attachments = dir.join(attachments::FOLDER);
            tauri::async_runtime::block_on(attachments::remove_unused(&pool, &attachments))?;
            let exe = std::env::current_exe()?;
            let backend = Arc::new(agent::Agents {
                claude: ClaudeBackend::new(&dir, &exe)?,
                opencode: OpenCodeBackend::new(&dir, &exe)?,
            });
            let notifier = Arc::new(events::TauriNotifier(app.handle().clone()));
            let agents = AgentManager::new(pool.clone(), backend, notifier);
            tauri::async_runtime::block_on(agents.recover())?;
            let kick = agents.clone();
            tauri::async_runtime::spawn(async move { kick.kick().await });
            app.manage(pool);
            app.manage(agents);
            app.manage(attachments::Attachments(attachments));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let agents = app.state::<Arc<AgentManager>>().inner().clone();
                tauri::async_runtime::block_on(agents.shutdown());
            }
        });
}

#[cfg(test)]
mod tests {
    /// Regenerates `src/lib/bindings.ts`; CI fails if the committed file is stale.
    #[test]
    fn export_bindings() {
        super::export_bindings(&super::specta_builder());
    }
}
