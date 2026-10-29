mod agent;
mod commands;
mod db;
mod error;
mod events;
mod models;
mod repo;

use std::sync::Arc;

use tauri::Manager;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands, collect_events};

use agent::claude::ClaudeBackend;
use agent::manager::AgentManager;

const BINDINGS_PATH: &str = "../src/lib/bindings.ts";

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::list_projects,
            commands::create_project,
            commands::update_project,
            commands::reorder_projects,
            commands::delete_project,
            commands::list_tickets,
            commands::create_ticket,
            commands::update_ticket,
            commands::move_ticket,
            commands::delete_ticket,
            commands::list_ticket_events,
            commands::reply_to_ticket,
            commands::send_back_to_ready,
            commands::cancel_ticket,
            commands::get_settings,
            commands::save_settings,
        ])
        .events(collect_events![events::TicketChanged, events::TicketEventAdded])
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

/// Entry point of `workzone mcp-ask-user`, which Claude Code starts as an MCP server.
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
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let pool = tauri::async_runtime::block_on(db::open(&dir.join("workzone.db")))?;
            let backend = Arc::new(ClaudeBackend::new(&dir, &std::env::current_exe()?)?);
            let notifier = Arc::new(events::TauriNotifier(app.handle().clone()));
            let agents = AgentManager::new(pool.clone(), backend, notifier);
            tauri::async_runtime::block_on(agents.recover())?;
            let kick = agents.clone();
            tauri::async_runtime::spawn(async move { kick.kick().await });
            app.manage(pool);
            app.manage(agents);
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
