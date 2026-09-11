use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::tmux::conf_dir;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub id: String,
    pub active_bg: String,
    pub active_fg: String,
    pub inactive_bg: String,
    pub inactive_fg: String,
    pub hint_bg: String,
    pub hint_fg: String,
    pub msg_bg: String,
    pub msg_fg: String,
    pub status_busy: String,
    pub status_attention: String,
    pub status_idle: String,
    pub status_default: String,
}

#[derive(Default, Deserialize)]
struct ThemeFile {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    active_bg: Option<String>,
    #[serde(default)]
    active_fg: Option<String>,
    #[serde(default)]
    inactive_bg: Option<String>,
    #[serde(default)]
    inactive_fg: Option<String>,
    #[serde(default)]
    hint_bg: Option<String>,
    #[serde(default)]
    hint_fg: Option<String>,
    #[serde(default)]
    msg_bg: Option<String>,
    #[serde(default)]
    msg_fg: Option<String>,
    #[serde(default)]
    status_busy: Option<String>,
    #[serde(default)]
    status_attention: Option<String>,
    #[serde(default)]
    status_idle: Option<String>,
    #[serde(default)]
    status_default: Option<String>,
}

impl Theme {
    fn apply_file(&mut self, f: ThemeFile, id: &str) {
        if let Some(n) = f.name {
            self.id = n;
        } else {
            self.id = id.to_string();
        }
        macro_rules! take {
            ($field:ident) => {
                if let Some(v) = f.$field {
                    if !v.trim().is_empty() {
                        self.$field = v;
                    }
                }
            };
        }
        take!(active_bg);
        take!(active_fg);
        take!(inactive_bg);
        take!(inactive_fg);
        take!(hint_bg);
        take!(hint_fg);
        take!(msg_bg);
        take!(msg_fg);
        take!(status_busy);
        take!(status_attention);
        take!(status_idle);
        take!(status_default);
    }
}

pub fn nord() -> Theme {
    Theme {
        id: "nord".into(),
        active_bg: "colour25".into(),
        active_fg: "colour231".into(),
        inactive_bg: "#3b4252".into(),
        inactive_fg: "#d8dee9".into(),
        hint_bg: "colour216".into(),
        hint_fg: "colour235".into(),
        msg_bg: "#2e3440".into(),
        msg_fg: "#d8dee9".into(),
        status_busy: "colour196".into(),
        status_attention: "colour221".into(),
        status_idle: "colour114".into(),
        status_default: "colour39".into(),
    }
}

pub fn catppuccin_mocha() -> Theme {
    Theme {
        id: "catppuccin-mocha".into(),
        active_bg: "#89b4fa".into(),
        active_fg: "#1e1e2e".into(),
        inactive_bg: "#313244".into(),
        inactive_fg: "#cdd6f4".into(),
        hint_bg: "#f9e2af".into(),
        hint_fg: "#1e1e2e".into(),
        msg_bg: "#1e1e2e".into(),
        msg_fg: "#cdd6f4".into(),
        status_busy: "#f38ba8".into(),
        status_attention: "#f9e2af".into(),
        status_idle: "#a6e3a1".into(),
        status_default: "#89b4fa".into(),
    }
}

pub fn tokyonight() -> Theme {
    Theme {
        id: "tokyonight".into(),
        active_bg: "#7aa2f7".into(),
        active_fg: "#1a1b26".into(),
        inactive_bg: "#24283b".into(),
        inactive_fg: "#c0caf5".into(),
        hint_bg: "#e0af68".into(),
        hint_fg: "#1a1b26".into(),
        msg_bg: "#1a1b26".into(),
        msg_fg: "#c0caf5".into(),
        status_busy: "#f7768e".into(),
        status_attention: "#e0af68".into(),
        status_idle: "#9ece6a".into(),
        status_default: "#7aa2f7".into(),
    }
}

pub fn gruvbox_dark() -> Theme {
    Theme {
        id: "gruvbox-dark".into(),
        active_bg: "#458588".into(),
        active_fg: "#ebdbb2".into(),
        inactive_bg: "#3c3836".into(),
        inactive_fg: "#ebdbb2".into(),
        hint_bg: "#d79921".into(),
        hint_fg: "#1d2021".into(),
        msg_bg: "#282828".into(),
        msg_fg: "#ebdbb2".into(),
        status_busy: "#fb4934".into(),
        status_attention: "#fabd2f".into(),
        status_idle: "#b8bb26".into(),
        status_default: "#83a598".into(),
    }
}

pub fn dracula() -> Theme {
    Theme {
        id: "dracula".into(),
        active_bg: "#bd93f9".into(),
        active_fg: "#282a36".into(),
        inactive_bg: "#44475a".into(),
        inactive_fg: "#f8f8f2".into(),
        hint_bg: "#ffb86c".into(),
        hint_fg: "#282a36".into(),
        msg_bg: "#282a36".into(),
        msg_fg: "#f8f8f2".into(),
        status_busy: "#ff5555".into(),
        status_attention: "#f1fa8c".into(),
        status_idle: "#50fa7b".into(),
        status_default: "#8be9fd".into(),
    }
}

pub fn solarized_dark() -> Theme {
    Theme {
        id: "solarized-dark".into(),
        active_bg: "#268bd2".into(),
        active_fg: "#fdf6e3".into(),
        inactive_bg: "#073642".into(),
        inactive_fg: "#93a1a1".into(),
        hint_bg: "#b58900".into(),
        hint_fg: "#002b36".into(),
        msg_bg: "#002b36".into(),
        msg_fg: "#93a1a1".into(),
        status_busy: "#dc322f".into(),
        status_attention: "#b58900".into(),
        status_idle: "#859900".into(),
        status_default: "#2aa198".into(),
    }
}

pub fn rose_pine() -> Theme {
    Theme {
        id: "rose-pine".into(),
        active_bg: "#c4a7e7".into(),
        active_fg: "#191724".into(),
        inactive_bg: "#26233a".into(),
        inactive_fg: "#e0def4".into(),
        hint_bg: "#f6c177".into(),
        hint_fg: "#191724".into(),
        msg_bg: "#191724".into(),
        msg_fg: "#e0def4".into(),
        status_busy: "#eb6f92".into(),
        status_attention: "#f6c177".into(),
        status_idle: "#9ccfd8".into(),
        status_default: "#c4a7e7".into(),
    }
}

pub fn builtins() -> Vec<Theme> {
    vec![
        nord(),
        catppuccin_mocha(),
        tokyonight(),
        gruvbox_dark(),
        dracula(),
        solarized_dark(),
        rose_pine(),
    ]
}

pub fn themes_dir() -> PathBuf {
    conf_dir().join("themes")
}

fn load_file(path: &Path) -> Option<Theme> {
    let raw = std::fs::read_to_string(path).ok()?;
    let parsed = if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml"))
        .unwrap_or(false)
    {
        serde_yaml::from_str::<ThemeFile>(&raw).ok()?
    } else {
        serde_json::from_str::<ThemeFile>(&raw).ok()?
    };
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom")
        .to_string();
    let mut t = nord();
    t.apply_file(parsed, &id);
    Some(t)
}

/// Resolve a theme id (`nord`) or a path to a json/yaml file.
pub fn load_theme(spec: &str) -> Theme {
    let spec = spec.trim();
    if spec.is_empty() {
        return nord();
    }
    for t in builtins() {
        if t.id == spec {
            return t;
        }
    }
    let path = PathBuf::from(spec);
    if path.is_file() {
        if let Some(t) = load_file(&path) {
            return t;
        }
    }
    let local = themes_dir().join(spec);
    if local.is_file() {
        if let Some(t) = load_file(&local) {
            return t;
        }
    }
    for ext in ["json", "yaml", "yml"] {
        let p = themes_dir().join(format!("{spec}.{ext}"));
        if p.is_file() {
            if let Some(t) = load_file(&p) {
                return t;
            }
        }
    }
    nord()
}

/// Builtin ids plus files in ~/.config/tabmux/themes.
pub fn list_theme_ids() -> Vec<String> {
    let mut ids: Vec<String> = builtins().into_iter().map(|t| t.id).collect();
    if let Ok(entries) = std::fs::read_dir(themes_dir()) {
        for e in entries.flatten() {
            let p = e.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "json" | "yaml" | "yml") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let id = stem.to_string();
                    if !ids.iter().any(|x| x == &id) {
                        ids.push(id);
                    }
                }
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::{load_theme, nord};

    #[test]
    fn builtin_nord() {
        assert_eq!(load_theme("nord").id, "nord");
        assert_eq!(load_theme("").id, "nord");
    }

    #[test]
    fn unknown_falls_back() {
        assert_eq!(load_theme("not-a-theme").inactive_bg, nord().inactive_bg);
    }
}
