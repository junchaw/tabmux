use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub const SOCKET: &str = "tabmux";
pub const ACTIVE_BG: &str = "colour25";
pub const ACTIVE_FG: &str = "colour231";
pub const INACTIVE_BG: &str = "#3b4252";
pub const INACTIVE_FG: &str = "#d8dee9";
pub const HINT: &str = "Try Ctrl+B";
pub const HINT_BG: &str = "colour216";
pub const HINT_FG: &str = "colour235";
pub const MSG_SEP: &str = " | ";
pub const MSG_BG: &str = "#2e3440";
pub const MSG_FG: &str = "#d8dee9";
pub const STATUS_BUSY_FG: &str = "colour221";
pub const STATUS_ATTENTION_FG: &str = "colour196";

pub fn conf_dir() -> PathBuf {
    dirs_next_home().join(".config/tabmux")
}

pub fn conf_path() -> PathBuf {
    conf_dir().join("tmux.conf")
}

pub fn msg_path() -> PathBuf {
    conf_dir().join("messages")
}

pub fn status_dir() -> PathBuf {
    conf_dir().join("status")
}

pub fn status_path(session: &str) -> PathBuf {
    status_dir().join(session)
}

pub fn sessions_path() -> PathBuf {
    conf_dir().join("sessions")
}

/// Working directory of a session's first pane, if it exists.
pub fn session_path(session: &str) -> Option<String> {
    let out = tmux_stdout(&[
        "display-message",
        "-p",
        "-t",
        &format!("={session}:0.0"),
        "#{pane_current_path}",
    ]);
    let out = out.trim();
    if out.is_empty() {
        None
    } else {
        Some(out.to_string())
    }
}

/// Session that owns the calling pane, when invoked from inside a tabmux pane.
pub fn current_session_from_pane() -> Option<String> {
    let pane = std::env::var("TMUX_PANE").ok()?;
    let out = tmux_stdout(&["display-message", "-p", "-t", &pane, "#{session_name}"]);
    let out = out.trim();
    if out.is_empty() {
        None
    } else {
        Some(out.to_string())
    }
}

fn dirs_next_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn launcher() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tabmux"))
}

pub fn tmux(args: &[&str]) -> Output {
    Command::new("tmux")
        .args(["-L", SOCKET])
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("tmux: {e}"))
}

pub fn tmux_ok(args: &[&str]) -> bool {
    tmux(args).status.success()
}

pub fn tmux_stdout(args: &[&str]) -> String {
    String::from_utf8_lossy(&tmux(args).stdout).to_string()
}

pub fn list_sessions() -> Vec<String> {
    let out = tmux_stdout(&["list-sessions", "-F", "#{session_created}\t#{session_name}"]);
    if !tmux(&["list-sessions"]).status.success() {
        return Vec::new();
    }
    let mut rows: Vec<(i64, String)> = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (ts, name) = line.split_once('\t').unwrap_or(("0", line));
        let created = ts.parse().unwrap_or(0);
        rows.push((created, name.to_string()));
    }
    rows.sort_by_key(|(ts, _)| *ts);
    rows.into_iter().map(|(_, n)| n).collect()
}

pub fn switch_to(session: &str, client: Option<&str>) {
    let target = format!("={session}");
    if let Some(c) = client.filter(|s| !s.is_empty()) {
        tmux(&["switch-client", "-c", c, "-t", &target]);
    } else {
        tmux(&["switch-client", "-t", &target]);
    }
}

pub fn unique_name() -> String {
    let existing = list_sessions();
    let mut i = 1;
    loop {
        let name = format!("s{i}");
        if !existing.iter().any(|s| s == &name) {
            return name;
        }
        i += 1;
    }
}

pub fn sty(bg: &str, fg: &str, text: &str, bold: bool) -> String {
    let b = if bold { ",bold" } else { "" };
    format!("#[bg={bg},fg={fg}{b}]{text}#[default]")
}

pub fn ensure_dir(p: &Path) {
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
}
