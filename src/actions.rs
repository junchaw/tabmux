use std::time::{SystemTime, UNIX_EPOCH};

use crate::tmux::{
    ensure_dir, list_sessions, msg_path, switch_to, tmux, tmux_ok, unique_name,
};

pub const MSG_KEEP: usize = 40;

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
    Some(prev)
}

pub fn cmd_nth(index: usize, client: Option<&str>) {
    let names = list_sessions();
    if index >= 1 && index <= names.len() {
        switch_to(&names[index - 1], client);
    }
}
