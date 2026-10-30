//! Images pasted into tickets and replies.
//!
//! A pasted image is saved as a file in the app data folder and referenced from the text as a
//! Markdown image with its absolute path, `![image](<C:/…/attachments/x.png>)`. The frontend shows
//! it through Tauri's asset protocol; agent backends attach every image a prompt references
//! (see [`referenced_images`]), and the path in the text lets the agent open the file itself.

use std::path::{Path, PathBuf};

use base64::Engine;
use sqlx::SqlitePool;

use crate::error::{AppResult, invalid};

/// Folder below the app data folder; also allowed in the asset protocol scope (`tauri.conf.json`).
pub const FOLDER: &str = "attachments";

/// Image types that agents accept, by file extension.
const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
];

/// The attachment folder; managed as Tauri state.
pub struct Attachments(pub PathBuf);

/// MIME type of an image file, `None` for other files.
pub fn media_type(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    IMAGE_TYPES.iter().find(|(e, _)| *e == ext).map(|(_, m)| *m)
}

/// Saves a base64-encoded image; returns its absolute path with forward slashes, which works
/// in Markdown and for the agents on every platform.
pub fn save(dir: &Path, data_base64: &str, extension: &str) -> AppResult<String> {
    let extension = extension.to_ascii_lowercase();
    if !IMAGE_TYPES.iter().any(|(e, _)| *e == extension) {
        return Err(invalid(format!("Unsupported image type: {extension}")));
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(data_base64.trim())
        .map_err(|e| invalid(format!("Invalid image data: {e}")))?;
    std::fs::create_dir_all(dir).map_err(|e| invalid(format!("Could not create {}: {e}", dir.display())))?;
    let now = chrono::Utc::now();
    let stem = format!("{}-{:09}", now.format("%Y%m%d-%H%M%S"), now.timestamp_subsec_nanos());
    let path = dir.join(format!("{stem}.{extension}"));
    std::fs::write(&path, data).map_err(|e| invalid(format!("Could not save the image: {e}")))?;
    Ok(path.to_string_lossy().replace('\\', "/"))
}

/// Existing local image files that `text` links to (`](<path>)` or `](path)`), in order,
/// without duplicates.
pub fn referenced_images(text: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for (i, _) in text.match_indices("](") {
        let rest = &text[i + 2..];
        let target = match rest.strip_prefix('<') {
            Some(r) => r.split('>').next(),
            None => rest.split([')', ' ', '\n']).next(),
        };
        let Some(path) = target.map(PathBuf::from) else { continue };
        if path.is_absolute() && media_type(&path).is_some() && path.is_file() && !found.contains(&path) {
            found.push(path);
        }
    }
    found
}

/// Deletes attachments that no ticket, pending reply or transcript refers to any more,
/// e.g. after their ticket was deleted. Run at start-up, when no unsaved draft can use them.
pub async fn remove_unused(db: &SqlitePool, dir: &Path) -> AppResult<()> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(()) };
    let texts: Vec<String> = sqlx::query_scalar(
        "SELECT body FROM tickets
         UNION ALL SELECT pending_prompt FROM tickets WHERE pending_prompt IS NOT NULL
         UNION ALL SELECT payload FROM ticket_events WHERE kind = 'user_prompt'",
    )
    .fetch_all(db)
    .await?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !texts.iter().any(|t| t.contains(&name)) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_images_and_finds_their_references() {
        let dir = std::env::temp_dir().join(format!("hivemind-attachments-{}", std::process::id()));
        let png = base64::engine::general_purpose::STANDARD.encode(b"not really a png");
        let a = save(&dir, &png, "PNG").unwrap();
        let b = save(&dir, &png, "jpg").unwrap();
        assert!(!a.contains('\\') && a.ends_with(".png"));
        assert!(save(&dir, &png, "exe").is_err());

        let text = format!(
            "Look: ![image](<{a}>) and ![b]({b})\nagain ![image](<{a}>), [missing](/no/such.png), [doc]({b}.txt)"
        );
        assert_eq!(referenced_images(&text), [PathBuf::from(&a), PathBuf::from(&b)]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
