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

/// Legacy single file, or the per-group directory once migrated.
fn messages_root() -> PathBuf {
    conf_dir().join("messages")
}

/// If `~/.config/tabmux/messages` is still a file, turn it into a directory
/// and move the old stream to the default group so history isn't lost.
pub fn migrate_messages() {
    let root = messages_root();
    if !root.is_file() {
        return;
    }
    let tmp = conf_dir().join(".messages.legacy");
    if std::fs::rename(&root, &tmp).is_err() {
        return;
    }
    let _ = std::fs::create_dir_all(&root);
    let _ = std::fs::rename(&tmp, root.join("default"));
}

/// Per-group event log. Group name is used as the filename.
pub fn msg_path(group: &str) -> PathBuf {
    migrate_messages();
    let group = group.trim();
    let group = if group.is_empty() || group.contains('/') || group == "." || group == ".." {
        "default"
    } else {
        group
    };
    messages_root().join(group)
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

pub fn groups_dir() -> PathBuf {
    conf_dir().join("groups")
}

pub fn group_path(group: &str) -> PathBuf {
    groups_dir().join(group)
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub created: i64,
}

/// Live sessions, oldest first. `id` is tmux's `#{session_id}` (`$0`, `$1`, …)
/// and survives `rename-session`.
pub fn list_session_rows() -> Vec<Session> {
    if !tmux(&["list-sessions"]).status.success() {
        return Vec::new();
    }
    let out = tmux_stdout(&[
        "list-sessions",
        "-F",
        "#{session_id}\t#{session_created}\t#{session_name}",
    ]);
    let mut rows = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let id = parts.next().unwrap_or("").to_string();
        let created = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let name = parts.next().unwrap_or("").to_string();
        if id.is_empty() || name.is_empty() {
            continue;
        }
        rows.push(Session { id, name, created });
    }
    rows.sort_by_key(|s| s.created);
    rows
}

pub fn list_sessions() -> Vec<String> {
    list_session_rows().into_iter().map(|s| s.name).collect()
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
