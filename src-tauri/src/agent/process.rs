//! Stopping an agent together with every process it started.
//!
//! Claude Code runs each shell command in its own session (`setsid`), so killing our process
//! group is not enough on Unix. Instead: ask Claude to stop (SIGTERM, it then stops its own
//! shells), and afterwards SIGKILL anything from a snapshot of its process tree that is still
//! alive. On Windows the group is a Job Object, which takes all descendants with it.

use std::time::Duration;

use command_group::AsyncGroupChild;

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
