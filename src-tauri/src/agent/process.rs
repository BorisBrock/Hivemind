//! Starting agent CLIs and stopping them together with every process they started.
//!
//! Claude Code runs each shell command in its own session (`setsid`), so killing our process
//! group is not enough on Unix. Instead: ask Claude to stop (SIGTERM, it then stops its own
//! shells), and afterwards SIGKILL anything from a snapshot of its process tree that is still
//! alive. On Windows the group is a Job Object, which takes all descendants with it.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use command_group::{AsyncCommandGroup, AsyncGroupChild};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::{AppError, AppResult};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(unix)]
const GRACE: Duration = Duration::from_secs(3);

pub async fn kill_tree(child: &mut AsyncGroupChild) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            let tree = descendants(pid);
            // SAFETY: plain kill(2) on a pid we spawned.
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            let _ = tokio::time::timeout(GRACE, child.wait()).await;
            for p in tree {
                unsafe { libc::kill(p as i32, libc::SIGKILL) };
            }
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// All descendants of `root`, from /proc. Empty where /proc is unavailable.
#[cfg(unix)]
fn descendants(root: u32) -> Vec<u32> {
    let Ok(dir) = std::fs::read_dir("/proc") else { return vec![] };
    let parents: Vec<(u32, u32)> = dir
        .filter_map(|e| {
            let pid: u32 = e.ok()?.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            // Format: "pid (comm) state ppid …"; comm may contain spaces, so split after ')'.
            let ppid = stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()?;
            Some((pid, ppid))
        })
        .collect();
    let mut found = vec![root];
    let mut i = 0;
    while i < found.len() {
        let parent = found[i];
        found.extend(parents.iter().filter(|(_, pp)| *pp == parent).map(|(p, _)| *p));
        i += 1;
    }
    found.remove(0);
    found
}

/// A spawned agent process with its pipes taken out.
pub struct Spawned {
    pub child: AsyncGroupChild,
    pub stdin: tokio::process::ChildStdin,
    pub stdout: tokio::process::ChildStdout,
    pub stderr: StderrTail,
}

/// Spawns `cmd` (in `cwd`) as a process group / Job Object with piped stdio and no console window.
pub fn spawn(label: &str, cwd: &Path, mut cmd: tokio::process::Command) -> AppResult<Spawned> {
    if !cwd.is_dir() {
        return Err(AppError::Invalid { message: format!("project folder not found: {}", cwd.display()) });
    }
    cmd.current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut group = cmd.group();
    #[cfg(windows)]
    group.creation_flags(CREATE_NO_WINDOW);
    let mut child =
        group.spawn().map_err(|e| AppError::Invalid { message: format!("could not start {label}: {e}") })?;
    let stdin = child.inner().stdin.take().expect("stdin is piped");
    let stdout = child.inner().stdout.take().expect("stdout is piped");
    let stderr = StderrTail::collect(child.inner().stderr.take().expect("stderr is piped"));
    Ok(Spawned { child, stdin, stdout, stderr })
}

/// A command for `binary`; npm's `.cmd` shims on Windows can only run through cmd.exe.
pub fn base_command(binary: &Path) -> tokio::process::Command {
    let is_script =
        binary.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if is_script {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(binary);
        c
    } else {
        tokio::process::Command::new(binary)
    }
}

/// The configured path, else `$env_var`, else `name` on PATH, else the first existing fallback.
pub fn find_binary(
    label: &str,
    configured: Option<&Path>,
    env_var: &str,
    name: &str,
    fallbacks: &[PathBuf],
    install_hint: &str,
) -> AppResult<PathBuf> {
    if let Some(path) = configured {
        return if path.is_file() {
            Ok(path.into())
        } else {
            Err(AppError::Invalid { message: format!("{label} not found at {}", path.display()) })
        };
    }
    if let Some(bin) = std::env::var_os(env_var) {
        return Ok(bin.into());
    }
    which::which(name).ok().or_else(|| fallbacks.iter().find(|p| p.is_file()).cloned()).ok_or_else(|| {
        AppError::Invalid {
            message: format!(
                "{label} was not found on PATH. Install it ({install_hint}) or set its path in Settings."
            ),
        }
    })
}

/// Runs `<binary> --version`; returns the first output line plus the path.
pub async fn version(binary: &Path) -> AppResult<String> {
    let mut cmd = base_command(binary);
    cmd.arg("--version");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = tokio::time::timeout(Duration::from_secs(15), cmd.output())
        .await
        .map_err(|_| AppError::Invalid { message: format!("{} --version did not answer", binary.display()) })?
        .map_err(|e| AppError::Invalid { message: format!("could not run {}: {e}", binary.display()) })?;
    let text = String::from_utf8_lossy(if out.status.success() { &out.stdout } else { &out.stderr });
    let line = text.lines().next().unwrap_or_default().trim().to_string();
    if out.status.success() {
        Ok(format!("{line} ({})", binary.display()))
    } else {
        Err(AppError::Invalid { message: format!("{} failed: {line}", binary.display()) })
    }
}

/// Keeps the last lines of an agent's stderr for error messages.
pub struct StderrTail(Arc<Mutex<VecDeque<String>>>);

impl StderrTail {
    fn collect(stderr: tokio::process::ChildStderr) -> Self {
        let lines = Arc::new(Mutex::new(VecDeque::new()));
        let sink = lines.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut t = sink.lock().unwrap();
                t.push_back(line);
                if t.len() > 20 {
                    t.pop_front();
                }
            }
        });
        Self(lines)
    }

    /// Error message for a run that ended badly, with the stderr tail.
    pub fn failure(&self, label: &str, status: std::io::Result<std::process::ExitStatus>) -> String {
        let reason = match status {
            Ok(s) => format!("{label} exited unexpectedly ({s})"),
            Err(e) => format!("{label} failed: {e}"),
        };
        let tail: Vec<String> = self.0.lock().unwrap().iter().cloned().collect();
        if tail.is_empty() { reason } else { format!("{reason}:\n{}", tail.join("\n")) }
    }
}
