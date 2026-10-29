mod commands;
mod db;
mod error;
mod models;
mod repo;

use tauri::Manager;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands};

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
            commands::get_settings,
            commands::save_settings,
        ])
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
            app.manage(pool);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    /// Regenerates `src/lib/bindings.ts`; CI fails if the committed file is stale.
    #[test]
    fn export_bindings() {
        super::export_bindings(&super::specta_builder());
    }
}
