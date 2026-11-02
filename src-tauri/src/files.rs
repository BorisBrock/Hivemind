//! Local files that agents mention in their messages.
//!
//! A path in an agent's message is not to be trusted: the agent may have read it in hostile repo
//! content. So the checks live here, not in the frontend. Only existing images can be previewed,
//! one file at a time, and only types that cannot run code open on a click; anything else is
//! merely shown in the file manager.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppResult, invalid};

/// Image types that can be previewed. Wider than what agents accept (`attachments::IMAGE_TYPES`).
/// An SVG shows in an `<img>`, where its scripts do not run.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"];

/// Further types that open in their default app. Must not hold anything that runs when opened,
/// which rules out source code (Windows runs `.js` and `.py`), HTML and Office files (macros).
const DOCUMENT_EXTENSIONS: &[&str] = &["pdf", "txt", "md", "log", "csv", "json", "yaml", "yml", "toml"];

/// A file that a message mentions.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalFile {
    /// The path as the message wrote it.
    pub mention: String,
    /// Absolute path.
    pub path: String,
    pub is_image: bool,
    /// A click opens it in its default app; otherwise it is shown in the file manager.
    pub can_open: bool,
}

fn extension_in(path: &Path, list: &[&str]) -> bool {
    // A Windows alternate data stream ("run.exe:x.txt") must not pass for its last extension.
    if cfg!(windows) && path.file_name().is_some_and(|n| n.to_string_lossy().contains(':')) {
        return false;
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    ext.is_some_and(|e| list.contains(&e.as_str()))
}

/// Finds the file a message means. `cwd` is the folder the agent works in; `home` expands `~/`.
/// Network paths are refused without touching them: on Windows, merely looking at
/// `\\host\share` sends the user's credentials to that host.
pub fn resolve(mention: &str, cwd: Option<&Path>, home: Option<&Path>) -> Option<LocalFile> {
    let slashed = mention.replace('\\', "/");
    if slashed.starts_with("//") {
        return None;
    }
    let path = match slashed.strip_prefix("~/") {
        Some(rest) => home?.join(rest),
        None if Path::new(mention).is_absolute() => PathBuf::from(mention),
        None => cwd?.join(mention),
    };
    // Classified by what the path really is: a link named `notes.txt` may point to a script.
    let real = std::fs::canonicalize(&path).ok()?;
    if !real.is_file() {
        return None;
    }
    let is_image = extension_in(&real, IMAGE_EXTENSIONS);
    Some(LocalFile {
        mention: mention.to_string(),
        path: path.to_string_lossy().replace('\\', "/"),
        is_image,
        can_open: is_image || extension_in(&real, DOCUMENT_EXTENSIONS),
    })
}

/// Whether a web link may be handed to the system.
pub fn is_web_url(url: &str) -> bool {
    let url = url.to_ascii_lowercase();
    ["http://", "https://", "mailto:"].iter().any(|s| url.starts_with(s))
}

/// Opens a file with its default app if its type is safe, else shows it in the file manager.
pub fn open(path: &str) -> AppResult<()> {
    let file = resolve(path, None, None).ok_or_else(|| invalid(format!("File not found: {path}")))?;
    if !file.can_open {
        return reveal(path);
    }
    tauri_plugin_opener::open_path(&file.path, None::<&str>)
        .map_err(|e| invalid(format!("Could not open {path}: {e}")))
}

/// Shows a file in the file manager, without opening it.
pub fn reveal(path: &str) -> AppResult<()> {
    let file = resolve(path, None, None).ok_or_else(|| invalid(format!("File not found: {path}")))?;
    tauri_plugin_opener::reveal_item_in_dir(&file.path)
        .map_err(|e| invalid(format!("Could not show {path}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hivemind-files-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_relative_absolute_and_home_paths() {
        let dir = temp_dir("resolve");
        std::fs::write(dir.join("circle.svg"), "<svg/>").unwrap();
        let absolute = dir.join("circle.svg").to_string_lossy().into_owned();

        let relative = resolve("circle.svg", Some(&dir), None).unwrap();
        assert!(relative.is_image && relative.can_open);
        assert_eq!(relative.mention, "circle.svg");
        assert!(Path::new(&relative.path).is_absolute());

        assert!(resolve(&absolute, None, None).is_some());
        assert!(resolve("~/circle.svg", None, Some(&dir)).is_some());
        // Nothing to resolve against, or nothing there.
        assert_eq!(resolve("circle.svg", None, None), None);
        assert_eq!(resolve("~/circle.svg", None, None), None);
        assert_eq!(resolve("missing.svg", Some(&dir), None), None);
        // Folders are not files.
        assert_eq!(resolve(&dir.to_string_lossy(), None, None), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_harmless_types_open() {
        let dir = temp_dir("types");
        for name in ["a.PNG", "b.pdf", "c.txt", "d.sh", "e.py", "f.exe", "g.html", "h.desktop", "Makefile"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let file = |name| resolve(name, Some(&dir), None).unwrap();
        assert!(file("a.PNG").is_image && file("a.PNG").can_open);
        for name in ["b.pdf", "c.txt"] {
            assert!(!file(name).is_image && file(name).can_open, "{name}");
        }
        for name in ["d.sh", "e.py", "f.exe", "g.html", "h.desktop", "Makefile"] {
            assert!(!file(name).is_image && !file(name).can_open, "{name}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn links_are_judged_by_their_target() {
        let dir = temp_dir("links");
        std::fs::write(dir.join("evil.sh"), "").unwrap();
        std::os::unix::fs::symlink(dir.join("evil.sh"), dir.join("notes.txt")).unwrap();
        std::os::unix::fs::symlink(dir.join("evil.sh"), dir.join("logo.png")).unwrap();
        for name in ["notes.txt", "logo.png"] {
            let file = resolve(name, Some(&dir), None).unwrap();
            assert!(!file.is_image && !file.can_open, "{name}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn network_paths_are_refused() {
        for path in [r"\\host\share\a.png", "//host/share/a.png", r"\/host/share/a.png"] {
            assert_eq!(resolve(path, Some(Path::new("/")), None), None, "{path}");
        }
    }

    #[test]
    fn only_web_links_go_to_the_system() {
        for url in ["https://example.com", "HTTP://example.com", "mailto:a@example.com"] {
            assert!(is_web_url(url), "{url}");
        }
        for url in ["file:///etc/passwd", "javascript:alert(1)", "smb://host/x", "/tmp/a.png"] {
            assert!(!is_web_url(url), "{url}");
        }
    }
}
