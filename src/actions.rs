use std::time::{SystemTime, UNIX_EPOCH};

use crate::tmux::{
    current_session_from_pane, ensure_dir, list_sessions, msg_path, status_path, switch_to, tmux,
    tmux_ok, unique_name,
};

pub const MSG_KEEP: usize = 40;
pub const STATUS_STATES: &[&str] = &["busy", "attention", "idle"];

pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn load_messages() -> Vec<(f64, String)> {
    let Ok(raw) = std::fs::read_to_string(msg_path()) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for ln in raw.lines() {
        let ln = ln.trim();
        if ln.is_empty() {
            continue;
        }
        if let Some((ts, text)) = ln.split_once('\t') {
            if let Ok(t) = ts.parse::<f64>() {
                items.push((t, text.to_string()));
                continue;
            }
        }
        items.push((now_secs(), ln.to_string()));
    }
    items
}

pub fn fmt_age(ts: f64) -> String {
    let sec = (now_secs() - ts).floor() as i64;
    let sec = sec.max(1);
    if sec < 60 {
        format!("{sec}s")
    } else {
        format!("{}m", sec / 60)
    }
}

pub fn start_flash(msg: &str, client: Option<&str>) {
    let msg = msg.trim();
    if msg.is_empty() {
        return;
    }
    let path = msg_path();
    ensure_dir(&path);
    let mut items = load_messages();
    items.push((now_secs(), msg.to_string()));
    let keep = if items.len() > MSG_KEEP {
        &items[items.len() - MSG_KEEP..]
    } else {
        &items[..]
    };
    let body: String = keep
        .iter()
        .map(|(ts, text)| format!("{ts}\t{text}\n"))
        .collect();
    let _ = std::fs::write(&path, body);
    if let Some(c) = client.filter(|s| !s.is_empty()) {
        tmux(&["refresh-client", "-S", "-t", c]);
    } else {
        tmux(&["refresh-client", "-S"]);
    }
}

pub fn cmd_new(name: &str, client: Option<&str>) -> (String, bool) {
    let mut name = name.trim().to_string();
    if name.is_empty() {
        name = unique_name();
    }
    let created = !tmux_ok(&["has-session", "-t", &format!("={name}")]);
    if created {
        tmux(&["new-session", "-d", "-s", &name]);
    }
    switch_to(&name, client);
    (name, created)
}

pub fn cmd_close(name: &str, client: Option<&str>) -> Option<String> {
    let names = list_sessions();
    let name = name.trim();
    if name.is_empty() || !names.iter().any(|s| s == name) {
        return None;
    }
    if names.len() <= 1 {
        return None;
    }
    let idx = names.iter().position(|s| s == name).unwrap();
    let prev = names[(idx + names.len() - 1) % names.len()].clone();
    if prev == name {
        return None;
    }
    switch_to(&prev, client);
    tmux(&["kill-session", "-t", &format!("={name}")]);
    let _ = std::fs::remove_file(status_path(name));
    Some(prev)
}

/// Reads the last-reported status for a session ("busy" / "attention"),
/// or None when idle / never reported.
pub fn read_status(session: &str) -> Option<String> {
    let raw = std::fs::read_to_string(status_path(session)).ok()?;
    let state = raw.trim().to_string();
    if state.is_empty() || state == "idle" {
        None
    } else {
        Some(state)
    }
}

pub fn cmd_status(state: &str, session: Option<&str>, client: Option<&str>) {
    let state = state.trim();
    if !STATUS_STATES.contains(&state) {
        eprintln!("unknown status {state:?}, expected one of {STATUS_STATES:?}");
        std::process::exit(2);
    }
    let Some(session) = session
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(current_session_from_pane)
    else {
        eprintln!("tabmux status: not inside a tabmux pane, pass a session name");
        std::process::exit(2);
    };
    let path = status_path(&session);
    if state == "idle" {
        let _ = std::fs::remove_file(&path);
    } else {
        ensure_dir(&path);
        let _ = std::fs::write(&path, state);
    }
    if let Some(c) = client.filter(|s| !s.is_empty()) {
        tmux(&["refresh-client", "-S", "-t", c]);
    } else {
        tmux(&["refresh-client", "-S"]);
    }
}

/// Renames `old` to `new_name`. Returns an error message on failure, None on success.
pub fn cmd_rename(old: &str, new_name: &str, _client: Option<&str>) -> Result<(), String> {
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err("name can't be empty".into());
    }
    if new_name == old {
        return Ok(());
    }
    if tmux_ok(&["has-session", "-t", &format!("={new_name}")]) {
        return Err(format!("{new_name} already exists"));
    }
    if !tmux_ok(&["rename-session", "-t", &format!("={old}"), new_name]) {
        return Err(format!("failed to rename {old}"));
    }
    if let Ok(state) = std::fs::read_to_string(status_path(old)) {
        let new_path = status_path(new_name);
        ensure_dir(&new_path);
        let _ = std::fs::write(&new_path, state);
    }
    let _ = std::fs::remove_file(status_path(old));
    Ok(())
}

pub fn cmd_nth(index: usize, client: Option<&str>) {
    let names = list_sessions();
    if index >= 1 && index <= names.len() {
        switch_to(&names[index - 1], client);
    }
}
