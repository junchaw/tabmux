use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use crate::tmux::conf_dir;

pub fn hooks_dir() -> std::path::PathBuf {
    conf_dir().join("hooks")
}

fn executable(path: &std::path::Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    meta.is_file() && meta.permissions().mode() & 0o111 != 0
}

fn spawn_hook(path: &std::path::Path, status: &str, prev: &str, session: &str) {
    if !executable(path) {
        return;
    }
    let _ = Command::new(path)
        .env("TABMUX_STATUS", status)
        .env("TABMUX_PREV_STATUS", prev)
        .env("TABMUX_SESSION", session)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Fire-and-forget user hooks after a status change.
///
/// Runs `~/.config/tabmux/hooks/status` (every change) and
/// `~/.config/tabmux/hooks/status-<name>` (that state only), if executable.
/// Skips when the new state matches the previous one.
pub fn run_status_hooks(status: &str, prev: Option<&str>, session: &str) {
    let prev = prev.unwrap_or("");
    if prev == status {
        return;
    }
    let dir = hooks_dir();
    spawn_hook(&dir.join("status"), status, prev, session);
    spawn_hook(&dir.join(format!("status-{status}")), status, prev, session);
}
