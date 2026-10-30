//! Carries data over from Workzone, this app's former name, the first time Hivemind starts.
//!
//! Workzone kept its files in folders named after its own bundle identifier. They are copied,
//! not moved, so the old folders stay behind as a backup.

use std::io;
use std::path::Path;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

const LEGACY_IDENTIFIER: &str = "dev.workzone.app";
const LEGACY_DB: &str = "workzone.db";
pub const DB: &str = "hivemind.db";
/// Written by `tauri-plugin-window-state`.
const WINDOW_STATE: &str = ".window-state.json";

/// Registered before the window-state plugin so that it already finds the migrated window state.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("workzone-migration")
        .setup(|app, _| {
            let data = app.path().app_data_dir()?;
            migrate_database(&legacy_dir(&data), &data)?;
            let config = app.path().app_config_dir()?;
            migrate_file(&legacy_dir(&config), &config, WINDOW_STATE)?;
            Ok(())
        })
        .build()
}

fn legacy_dir(dir: &Path) -> std::path::PathBuf {
    dir.with_file_name(LEGACY_IDENTIFIER)
}

/// Copies the database with its WAL files unless Hivemind already has one. The main file goes
/// last, so an interrupted copy is simply redone on the next start.
fn migrate_database(from: &Path, to: &Path) -> io::Result<()> {
    if to.join(DB).exists() || !from.join(LEGACY_DB).exists() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for suffix in ["-wal", "-shm"] {
        let old = from.join(format!("{LEGACY_DB}{suffix}"));
        let new = to.join(format!("{DB}{suffix}"));
        if old.exists() {
            std::fs::copy(old, new)?;
        } else if new.exists() {
            std::fs::remove_file(new)?;
        }
    }
    std::fs::copy(from.join(LEGACY_DB), to.join(DB))?;
    Ok(())
}

fn migrate_file(from: &Path, to: &Path, name: &str) -> io::Result<()> {
    if to.join(name).exists() || !from.join(name).exists() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    std::fs::copy(from.join(name), to.join(name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("hivemind-legacy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copies_database_with_wal_files_and_window_state() {
        let base = temp_dir("copy");
        let (old, new) = (base.join(LEGACY_IDENTIFIER), base.join("dev.hivemind.app"));
        std::fs::create_dir_all(&old).unwrap();
        for (name, content) in [("workzone.db", "db"), ("workzone.db-wal", "wal"), (WINDOW_STATE, "{}")] {
            std::fs::write(old.join(name), content).unwrap();
        }
        assert_eq!(legacy_dir(&new), old);

        migrate_database(&old, &new).unwrap();
        migrate_file(&old, &new, WINDOW_STATE).unwrap();

        assert_eq!(std::fs::read_to_string(new.join("hivemind.db")).unwrap(), "db");
        assert_eq!(std::fs::read_to_string(new.join("hivemind.db-wal")).unwrap(), "wal");
        assert!(!new.join("hivemind.db-shm").exists());
        assert_eq!(std::fs::read_to_string(new.join(WINDOW_STATE)).unwrap(), "{}");
        assert!(old.join("workzone.db").exists(), "the old data stays as a backup");
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn never_overwrites_existing_hivemind_data() {
        let base = temp_dir("keep");
        let (old, new) = (base.join(LEGACY_IDENTIFIER), base.join("dev.hivemind.app"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("workzone.db"), "old").unwrap();
        std::fs::write(old.join("workzone.db-wal"), "old").unwrap();
        std::fs::write(new.join("hivemind.db"), "new").unwrap();

        migrate_database(&old, &new).unwrap();

        assert_eq!(std::fs::read_to_string(new.join("hivemind.db")).unwrap(), "new");
        assert!(!new.join("hivemind.db-wal").exists());
        std::fs::remove_dir_all(base).unwrap();
    }
}
