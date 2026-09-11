use crate::groups::group_of_session;
use crate::tmux::{
    conf_dir, ensure_dir, launcher, list_sessions, tmux, INACTIVE_BG, MSG_BG,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pos {
    Top,
    Bottom,
}

impl Pos {
    pub fn as_str(self) -> &'static str {
        match self {
            Pos::Top => "top",
            Pos::Bottom => "bottom",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "top" => Some(Pos::Top),
            "bottom" => Some(Pos::Bottom),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarConfig {
    pub events: Pos,
    pub tabs: Pos,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            events: Pos::Top,
            tabs: Pos::Bottom,
        }
    }
}

fn global_path() -> std::path::PathBuf {
    conf_dir().join("config")
}

fn group_dir() -> std::path::PathBuf {
    conf_dir().join("config.groups")
}

fn group_path(group: &str) -> std::path::PathBuf {
    let g = group.trim();
    let g = if g.is_empty() || g.contains('/') || g == "." || g == ".." {
        "default"
    } else {
        g
    };
    group_dir().join(g)
}

fn parse_file(raw: &str) -> (BarConfig, bool) {
    let mut cfg = BarConfig::default();
    let mut setup = false;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "events" => {
                if let Some(p) = Pos::parse(v) {
                    cfg.events = p;
                }
            }
            "tabs" => {
                if let Some(p) = Pos::parse(v) {
                    cfg.tabs = p;
                }
            }
            "setup_done" => {
                setup = matches!(v.trim(), "1" | "true" | "yes");
            }
            _ => {}
        }
    }
    (cfg, setup)
}

fn format_file(cfg: &BarConfig, setup_done: bool) -> String {
    let mut s = format!("events={}\ntabs={}\n", cfg.events.as_str(), cfg.tabs.as_str());
    if setup_done {
        s.push_str("setup_done=true\n");
    }
    s
}

pub fn load_global() -> BarConfig {
    let Ok(raw) = std::fs::read_to_string(global_path()) else {
        return BarConfig::default();
    };
    parse_file(&raw).0
}

pub fn setup_done() -> bool {
    let Ok(raw) = std::fs::read_to_string(global_path()) else {
        return false;
    };
    parse_file(&raw).1
}

pub fn save_global(cfg: BarConfig) {
    let done = true;
    let path = global_path();
    ensure_dir(&path);
    let _ = std::fs::write(&path, format_file(&cfg, done));
}

pub fn reset_global() {
    let _ = std::fs::remove_file(global_path());
}

pub fn load_group_override(group: &str) -> Option<BarConfig> {
    let raw = std::fs::read_to_string(group_path(group)).ok()?;
    Some(parse_file(&raw).0)
}

pub fn group_has_override(group: &str) -> bool {
    group_path(group).is_file()
}

pub fn save_group_override(group: &str, cfg: BarConfig) {
    let path = group_path(group);
    ensure_dir(&path);
    let _ = std::fs::write(&path, format_file(&cfg, false));
}

pub fn remove_group_override(group: &str) {
    let _ = std::fs::remove_file(group_path(group));
}

pub fn resolved(group: Option<&str>) -> BarConfig {
    if let Some(g) = group.filter(|s| !s.is_empty()) {
        if let Some(over) = load_group_override(g) {
            return over;
        }
    }
    load_global()
}

pub fn resolved_for_session(session: &str) -> BarConfig {
    let group = group_of_session(session);
    resolved(group.as_deref())
}

/// Events bar is on the tmux status line (not pane-border) at `line`.
pub fn status_line_is_events(cfg: &BarConfig, line: i32) -> bool {
    if cfg.tabs != cfg.events {
        return false;
    }
    if cfg.tabs == Pos::Top {
        line == 0
    } else {
        line >= 1
    }
}

/// Default tmux.conf snippet from the global bar config.
pub fn tmux_status_block(exe: &str) -> String {
    let cfg = load_global();
    let tabs_fmt = format!(
        "set -g status-format[0] \"#[align=left fill={INACTIVE_BG}]#({exe} render #{{client_width}} #{{q:session_name}})\""
    );
    let ev_fmt = format!(
        "#[align=left fill={MSG_BG}]#({exe} render-msgs #{{client_width}} #{{q:session_name}})"
    );
    if cfg.tabs == cfg.events {
        let pos = cfg.tabs.as_str();
        if cfg.tabs == Pos::Bottom {
            format!(
                "set -g status 2\nset -g status-position {pos}\n{tabs_fmt}\nset -g status-format[1] \"{ev_fmt}\"\nset -g pane-border-status off\n"
            )
        } else {
            format!(
                "set -g status 2\nset -g status-position {pos}\nset -g status-format[0] \"{ev_fmt}\"\nset -g status-format[1] \"#[align=left fill={INACTIVE_BG}]#({exe} render #{{client_width}} #{{q:session_name}})\"\nset -g pane-border-status off\n"
            )
        }
    } else {
        format!(
            "set -g status on\nset -g status-position {tabs}\n{tabs_fmt}\nset -g pane-border-status {events}\nset -g pane-border-style \"bg={MSG_BG},fg={MSG_BG}\"\nset -g pane-active-border-style \"bg={MSG_BG},fg={MSG_BG}\"\nset -g pane-border-format \"{ev_fmt}\"\n",
            tabs = cfg.tabs.as_str(),
            events = cfg.events.as_str(),
        )
    }
}

fn window_targets(session: &str) -> Vec<String> {
    let out = crate::tmux::tmux_stdout(&[
        "list-windows",
        "-t",
        session,
        "-F",
        "#{session_name}:#{window_index}",
    ]);
    let mut v: Vec<String> = out
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if v.is_empty() {
        v.push(format!("{session}:0"));
    }
    v
}

pub fn apply_layout(session: &str) {
    let session = session.trim();
    if session.is_empty() {
        return;
    }
    let cfg = resolved_for_session(session);
    let exe = launcher().display().to_string();
    let tabs_fmt = format!(
        "#[align=left fill={INACTIVE_BG}]#({exe} render #{{client_width}} #{{q:session_name}})"
    );
    let ev_fmt = format!(
        "#[align=left fill={MSG_BG}]#({exe} render-msgs #{{client_width}} #{{q:session_name}})"
    );
    let wins = window_targets(session);

    // Drop leftover 2nd status line / pane-border from the previous layout.
    tmux(&["set-option", "-t", session, "status-format[1]", ""]);
    for w in &wins {
        tmux(&["set-option", "-w", "-t", w, "pane-border-status", "off"]);
    }
    tmux(&["set-option", "-t", session, "window-status-format", ""]);
    tmux(&["set-option", "-t", session, "window-status-current-format", ""]);

    if cfg.tabs == cfg.events {
        tmux(&["set-option", "-t", session, "status", "2"]);
        tmux(&["set-option", "-t", session, "status-position", cfg.tabs.as_str()]);
        for w in &wins {
            tmux(&["set-option", "-w", "-t", w, "pane-border-status", "off"]);
        }
        if cfg.tabs == Pos::Bottom {
            tmux(&["set-option", "-t", session, "status-format[0]", &tabs_fmt]);
            tmux(&["set-option", "-t", session, "status-format[1]", &ev_fmt]);
        } else {
            tmux(&["set-option", "-t", session, "status-format[0]", &ev_fmt]);
            tmux(&["set-option", "-t", session, "status-format[1]", &tabs_fmt]);
        }
    } else {
        tmux(&["set-option", "-t", session, "status", "on"]);
        tmux(&["set-option", "-t", session, "status-position", cfg.tabs.as_str()]);
        tmux(&["set-option", "-t", session, "status-format[0]", &tabs_fmt]);
        let style = format!("bg={MSG_BG},fg={MSG_BG}");
        tmux(&["set-option", "-t", session, "pane-border-style", &style]);
        tmux(&["set-option", "-t", session, "pane-active-border-style", &style]);
        for w in &wins {
            tmux(&[
                "set-option",
                "-w",
                "-t",
                w,
                "pane-border-status",
                cfg.events.as_str(),
            ]);
            tmux(&["set-option", "-w", "-t", w, "pane-border-format", &ev_fmt]);
        }
    }
    tmux(&["refresh-client", "-S"]);
}

pub fn apply_all_layouts() {
    for name in list_sessions() {
        apply_layout(&name);
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_file, BarConfig, Pos};

    #[test]
    fn parse_defaults() {
        let (cfg, setup) = parse_file("");
        assert_eq!(cfg, BarConfig::default());
        assert!(!setup);
    }

    #[test]
    fn parse_values() {
        let (cfg, setup) = parse_file("events=bottom\ntabs=top\nsetup_done=true\n");
        assert_eq!(cfg.events, Pos::Bottom);
        assert_eq!(cfg.tabs, Pos::Top);
        assert!(setup);
    }
}
