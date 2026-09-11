use std::io::{self, Write};

use crate::actions::{apply_status, cmd_new, cmd_nth, cmd_rename, cmd_step, cmd_step_status, read_status, start_flash, status_choices, status_fg};
use crate::groups::{
    close_session, group_of_session, members_for, rename_member, set_member_order, stamp_group_ids,
    CloseOutcome, DEFAULT_GROUP,
};
use crate::tmux::{launcher, tmux, unique_name};

pub fn popup_menu(client: Option<&str>) {
    let exe = launcher().display().to_string();
    let mut args: Vec<&str> = vec!["display-popup", "-B", "-w", "100%", "-h", "100%", "-E"];
    let cmd = match client.filter(|s| !s.is_empty()) {
        Some(c) => {
            args = vec!["display-popup", "-c", c, "-B", "-w", "100%", "-h", "100%", "-E"];
            format!("{exe} menu {c}")
        }
        None => format!("{exe} menu"),
    };
    let mut v = args;
    v.push(&cmd);
    // lifetimes: cmd must live. rebuild:
    let _ = v;
    if let Some(c) = client.filter(|s| !s.is_empty()) {
        let cmd = format!("{exe} menu {c}");
        tmux(&[
            "display-popup",
            "-c",
            c,
            "-B",
            "-w",
            "100%",
            "-h",
            "100%",
            "-E",
            &cmd,
        ]);
    } else {
        let cmd = format!("{exe} menu");
        tmux(&[
            "display-popup",
            "-B",
            "-w",
            "100%",
            "-h",
            "100%",
            "-E",
            &cmd,
        ]);
    }
}

enum Key {
    Char(char),
    Up,
    Down,
    Enter,
    Esc,
    Other,
}

fn read_byte(timeout_tenths: Option<u8>) -> Option<u8> {
    unsafe {
        let fd = 0;
        let mut old: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut old) != 0 {
            return None;
        }
        let mut raw = old;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        match timeout_tenths {
            Some(t) => {
                raw.c_cc[libc::VMIN] = 0;
                raw.c_cc[libc::VTIME] = t;
            }
            None => {
                raw.c_cc[libc::VMIN] = 1;
                raw.c_cc[libc::VTIME] = 0;
            }
        }
        if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
            return None;
        }
        let mut buf = [0u8; 1];
        let n = libc::read(fd, buf.as_mut_ptr() as *mut _, 1);
        libc::tcsetattr(fd, libc::TCSANOW, &old);
        if n == 1 {
            Some(buf[0])
        } else {
            None
        }
    }
}

fn read_raw() -> Option<u8> {
    read_byte(None)
}

fn read_key() -> Key {
    let Some(b) = read_byte(None) else {
        return Key::Other;
    };
    match b {
        b'\r' | b'\n' => Key::Enter,
        0x03 | 0x04 => Key::Esc,
        0x1b => match read_byte(Some(1)) {
            Some(b'[') => match read_byte(Some(1)) {
                Some(b'A') | Some(b'D') => Key::Up,
                Some(b'B') | Some(b'C') => Key::Down,
                _ => Key::Esc,
            },
            _ => Key::Esc,
        },
        c if c.is_ascii_graphic() || c == b' ' => Key::Char(c as char),
        _ => Key::Other,
    }
}

fn term_size() -> (usize, usize) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
            (ws.ws_col as usize, ws.ws_row as usize)
        } else {
            (80, 24)
        }
    }
}

fn clip_pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.chars().take(w).collect()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

fn box_rule(left: char, join: char, right: char, widths: &[usize]) -> String {
    let mut s = String::from("\x1b[38;5;245m");
    s.push(left);
    for (i, w) in widths.iter().enumerate() {
        if i > 0 {
            s.push(join);
        }
        s.extend(std::iter::repeat('─').take(w + 2));
    }
    s.push(right);
    s.push_str("\x1b[0m\n");
    s
}

fn box_row(cells: &[(&str, usize, bool)]) -> String {
    let mut s = String::from("\x1b[38;5;245m│\x1b[0m");
    for (text, w, bold) in cells {
        let body = clip_pad(text, *w);
        s.push(' ');
        if *bold && !text.is_empty() {
            s.push_str("\x1b[1m");
            s.push_str(&body);
            s.push_str("\x1b[0m");
        } else {
            s.push_str(&body);
        }
        s.push(' ');
        s.push_str("\x1b[38;5;245m│\x1b[0m");
    }
    s.push('\n');
    s
}

fn render_menu_table(left: &[(String, String)], right: &[(String, String)], cols: usize) -> (String, usize) {
    let key_w = left
        .iter()
        .chain(right.iter())
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(1)
        .max("key".len());
    let act_w = |items: &[(String, String)]| {
        items
            .iter()
            .map(|(_, d)| d.chars().count())
            .max()
            .unwrap_or(0)
            .max("action".len())
    };
    let aw_l = act_w(left);
    let aw_r = act_w(right);
    let two_w = 2 * key_w + aw_l + aw_r + 13;
    let one_w = |a: usize| key_w + a + 7;
    let two = two_w <= cols;
    let mut out = String::new();
    if two {
        let widths = [key_w, aw_l, key_w, aw_r];
        out.push_str(&box_rule('┌', '┬', '┐', &widths));
        out.push_str(&box_row(&[
            ("key", key_w, false),
            ("action", aw_l, false),
            ("key", key_w, false),
            ("action", aw_r, false),
        ]));
        out.push_str(&box_rule('├', '┼', '┤', &widths));
        let n = left.len().max(right.len());
        for i in 0..n {
            let (lk, ld) = left.get(i).map(|(k, d)| (k.as_str(), d.as_str())).unwrap_or(("", ""));
            let (rk, rd) = right.get(i).map(|(k, d)| (k.as_str(), d.as_str())).unwrap_or(("", ""));
            out.push_str(&box_row(&[
                (lk, key_w, true),
                (ld, aw_l, false),
                (rk, key_w, true),
                (rd, aw_r, false),
            ]));
        }
        out.push_str(&box_rule('└', '┴', '┘', &widths));
        return (out, two_w);
    } else {
        let mut rows: Vec<(&str, &str)> = left.iter().map(|(k, d)| (k.as_str(), d.as_str())).collect();
        rows.push(("", ""));
        rows.extend(right.iter().map(|(k, d)| (k.as_str(), d.as_str())));
        let mut aw = rows.iter().map(|(_, d)| d.chars().count()).max().unwrap_or(0).max("action".len());
        if one_w(aw) > cols {
            aw = cols.saturating_sub(key_w + 7).max("action".len());
        }
        let widths = [key_w, aw];
        out.push_str(&box_rule('┌', '┬', '┐', &widths));
        out.push_str(&box_row(&[("key", key_w, false), ("action", aw, false)]));
        out.push_str(&box_rule('├', '┼', '┤', &widths));
        for (k, d) in rows {
            out.push_str(&box_row(&[(k, key_w, true), (d, aw, false)]));
        }
        out.push_str(&box_rule('└', '┴', '┘', &widths));
        (out, one_w(aw))
    }
}

fn prompt_new_session_name(group: &str) -> String {
    let default = unique_name(Some(group));
    prompt_text("new session", &format!("empty uses {default}"))
}

fn prompt_text(title: &str, hint: &str) -> String {
    let mut buf = String::new();
    loop {
        let (cols, rows) = term_size();
        let box_w = 44.min(28.max(cols.saturating_sub(8)).max(hint.len() + 2));
        let mut shown = buf.clone();
        if shown.chars().count() > box_w.saturating_sub(9) {
            shown = shown.chars().rev().take(box_w.saturating_sub(9)).collect::<String>();
            shown = shown.chars().rev().collect();
        }
        let field = format!(" name: {shown}_");
        let lines = [
            format!("┌{}┐", "─".repeat(box_w)),
            format!("│{:^width$}│", title, width = box_w),
            format!("│{}│", " ".repeat(box_w)),
            format!("│{:<width$}│", field.chars().take(box_w).collect::<String>(), width = box_w),
            format!("│{:^width$}│", hint, width = box_w),
            format!("└{}┘", "─".repeat(box_w)),
        ];
        let r0 = 1.max((rows.saturating_sub(lines.len())) / 2 + 1);
        let c0 = 1.max((cols.saturating_sub(box_w + 2)) / 2 + 1);
        let mut out = String::from("\x1b[H\x1b[J");
        for (i, line) in lines.iter().enumerate() {
            out.push_str(&format!("\x1b[{};{}H{line}", r0 + i, c0));
        }
        let _ = io::stdout().write_all(out.as_bytes());
        let _ = io::stdout().flush();
        let Some(b) = read_raw() else {
            continue;
        };
        match b {
            b'\r' | b'\n' => return buf.trim().to_string(),
            0x7f | 0x08 => {
                buf.pop();
            }
            0x1b | 0x03 | 0x04 => {}
            c if c.is_ascii_graphic() || c == b' ' => buf.push(c as char),
            _ => {}
        }
    }
}

fn client_session(client: Option<&str>) -> String {
    let out = if let Some(c) = client.filter(|s| !s.is_empty()) {
        crate::tmux::tmux_stdout(&["display-message", "-c", c, "-p", "#{session_name}"])
    } else {
        crate::tmux::tmux_stdout(&["display-message", "-p", "#{session_name}"])
    };
    out.trim().to_string()
}

fn refresh_bar(client: Option<&str>) {
    if let Some(c) = client.filter(|s| !s.is_empty()) {
        tmux(&["refresh-client", "-S", "-t", c]);
    } else {
        tmux(&["refresh-client", "-S"]);
    }
}

fn draw_pick(title: &str, items: &[&str], idx: usize) {
    let (cols, rows) = term_size();
    let footer = "j/k or arrows  select · enter  confirm · esc  cancel";
    let header_rows = 3;
    let footer_rows = 2;
    let vis = rows.saturating_sub(header_rows + footer_rows).max(1);
    let start = if idx >= vis { idx + 1 - vis } else { 0 };
    let mut out = String::from("\x1b[H\x1b[J");
    out.push_str(&format!(
        "\x1b[38;5;216;1mtabmux\x1b[0m  \x1b[38;5;252m{title}\x1b[0m\n\n"
    ));
    for (i, name) in items.iter().enumerate().skip(start).take(vis) {
        let mark = if i == idx { "▸" } else { " " };
        let body = pad_line(&format!("  {mark}  {name}"), cols);
        let line = if i == idx {
            format!("\x1b[1m{body}\x1b[0m")
        } else {
            format!("\x1b[38;5;252m{body}\x1b[0m")
        };
        out.push_str(&line);
        out.push('\n');
    }
    let r_footer = rows.max(footer_rows);
    out.push_str(&format!("\x1b[{r_footer};1H\x1b[38;5;245m{footer}\x1b[0m"));
    let _ = io::stdout().write_all(out.as_bytes());
    let _ = io::stdout().flush();
}

fn pick_list(title: &str, items: &[&str], start: usize) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let mut idx = start.min(items.len() - 1);
    loop {
        draw_pick(title, items, idx);
        match read_key() {
            Key::Char('j') | Key::Char('J') | Key::Down => {
                if idx + 1 < items.len() {
                    idx += 1;
                }
            }
            Key::Char('k') | Key::Char('K') | Key::Up => {
                if idx > 0 {
                    idx -= 1;
                }
            }
            Key::Enter => return Some(idx),
            Key::Esc | Key::Char('q') => return None,
            _ => {}
        }
    }
}

fn pick_pos(title: &str, current: crate::config::Pos) -> Option<crate::config::Pos> {
    let start = if current == crate::config::Pos::Bottom { 1 } else { 0 };
    match pick_list(title, &["top", "bottom"], start)? {
        0 => Some(crate::config::Pos::Top),
        _ => Some(crate::config::Pos::Bottom),
    }
}

fn ansi_bg(tmux_color: &str) -> String {
    let c = tmux_color.trim();
    if let Some(n) = c.strip_prefix("colour") {
        return format!("\x1b[48;5;{n}m");
    }
    if c.starts_with('#') && c.len() >= 7 {
        let r = u8::from_str_radix(&c[1..3], 16).unwrap_or(40);
        let g = u8::from_str_radix(&c[3..5], 16).unwrap_or(40);
        let b = u8::from_str_radix(&c[5..7], 16).unwrap_or(40);
        return format!("\x1b[48;2;{r};{g};{b}m");
    }
    "\x1b[48;5;236m".into()
}

fn paint_seg(bg: &str, fg: &str, text: &str, bold: bool) -> String {
    format!(
        "{}{}{}{}\x1b[0m",
        ansi_bg(bg),
        ansi_fg(fg),
        if bold { "\x1b[1m" } else { "" },
        text
    )
}

fn draw_theme_preview(theme: &crate::theme::Theme, cols: usize) -> String {
    let hint = " Try Ctrl+B ";
    let hint_n = hint.chars().count();
    let rest_n = cols.saturating_sub(hint_n);
    let mut events = paint_seg(&theme.hint_bg, &theme.hint_fg, hint, true);
    let msg = if rest_n > 0 {
        let sample = "2m created pi agent | 4s switched to main";
        let n = sample.chars().count();
        let s = if n >= rest_n {
            sample.chars().skip(n - rest_n).collect::<String>()
        } else {
            format!("{}{sample}", " ".repeat(rest_n - n))
        };
        paint_seg(&theme.msg_bg, &theme.msg_fg, &s, false)
    } else {
        String::new()
    };
    events.push_str(&msg);
    events.push('\n');

    let cells = [
        ("● main (1)", true, Some(&theme.status_idle)),
        ("● tabmux (2)", false, Some(&theme.status_busy)),
        ("● pi agent (3)", false, Some(&theme.status_attention)),
    ];
    let n = cells.len();
    let cell_w = if n == 0 { cols } else { cols / n };
    let rem = if n == 0 { 0 } else { cols % n };
    let mut tabs = String::new();
    for (i, (label, active, dot)) in cells.iter().enumerate() {
        let w = cell_w + if i < rem { 1 } else { 0 };
        let (bg, fg, bold) = if *active {
            (theme.active_bg.as_str(), theme.active_fg.as_str(), true)
        } else {
            (theme.inactive_bg.as_str(), theme.inactive_fg.as_str(), false)
        };
        let fitted = {
            let n = label.chars().count();
            if n >= w {
                label.chars().take(w).collect::<String>()
            } else {
                let left = (w - n) / 2;
                let right = w - n - left;
                format!("{}{}{}", " ".repeat(left), label, " ".repeat(right))
            }
        };
        if let Some(dot_fg) = dot {
            if let Some(pos) = fitted.find('●') {
                let (before, rest) = fitted.split_at(pos);
                let after = rest.get(char::len_utf8('●')..).unwrap_or("");
                tabs.push_str(&paint_seg(bg, fg, before, bold));
                tabs.push_str(&paint_seg(bg, dot_fg, "●", bold));
                tabs.push_str(&paint_seg(bg, fg, after, bold));
                continue;
            }
        }
        tabs.push_str(&paint_seg(bg, fg, &fitted, bold));
    }
    format!("{events}{tabs}\n")
}

fn draw_theme_pick(ids: &[String], idx: usize) {
    let (cols, rows) = term_size();
    let footer = "j/k  select · enter  confirm · esc  cancel";
    let preview_lines = 4;
    let header_rows = 2;
    let footer_rows = 2;
    let vis = rows
        .saturating_sub(header_rows + preview_lines + footer_rows)
        .max(1);
    let start = if idx >= vis { idx + 1 - vis } else { 0 };
    let mut out = String::from("\x1b[H\x1b[J");
    out.push_str("\x1b[38;5;216;1mtabmux\x1b[0m  \x1b[38;5;252mtheme\x1b[0m\n\n");
    let preview_id = ids.get(idx).map(|s| s.as_str()).unwrap_or("nord");
    if preview_id != "from file…" {
        let th = crate::theme::load_theme(preview_id);
        out.push_str(&draw_theme_preview(&th, cols));
        out.push('\n');
    } else {
        out.push_str("\x1b[38;5;245m  (pick a json/yaml file next)\x1b[0m\n\n\n\n");
    }
    for (i, name) in ids.iter().enumerate().skip(start).take(vis) {
        let mark = if i == idx { "▸" } else { " " };
        let body = pad_line(&format!("  {mark}  {name}"), cols);
        let line = if i == idx {
            format!("\x1b[1m{body}\x1b[0m")
        } else {
            format!("\x1b[38;5;252m{body}\x1b[0m")
        };
        out.push_str(&line);
        out.push('\n');
    }
    let r_footer = rows.max(footer_rows);
    out.push_str(&format!("\x1b[{r_footer};1H\x1b[38;5;245m{footer}\x1b[0m"));
    let _ = io::stdout().write_all(out.as_bytes());
    let _ = io::stdout().flush();
}

fn pick_theme(current: &str) -> Option<String> {
    let mut ids = crate::theme::list_theme_ids();
    ids.push("from file…".into());
    let start = ids.iter().position(|s| s == current).unwrap_or(0);
    let mut idx = start.min(ids.len() - 1);
    loop {
        draw_theme_pick(&ids, idx);
        match read_key() {
            Key::Char('j') | Key::Char('J') | Key::Down => {
                if idx + 1 < ids.len() {
                    idx += 1;
                }
            }
            Key::Char('k') | Key::Char('K') | Key::Up => {
                if idx > 0 {
                    idx -= 1;
                }
            }
            Key::Enter => {
                if idx + 1 == ids.len() {
                    let path = prompt_text("theme file", "json or yaml path");
                    if path.is_empty() {
                        return None;
                    }
                    return Some(path);
                }
                return Some(ids[idx].clone());
            }
            Key::Esc | Key::Char('q') => return None,
            _ => {}
        }
    }
}

/// First-run wizard: theme, then bar positions.
pub fn cmd_getting_started() {
    let mut cfg = crate::config::load_global();
    if let Some(theme) = pick_theme(&cfg.theme) {
        cfg.theme = theme;
    } else {
        crate::config::save_global(cfg);
        return;
    }
    let Some(events) = pick_pos("where is the Ctrl+B / events bar?", cfg.events) else {
        crate::config::save_global(cfg);
        return;
    };
    cfg.events = events;
    let Some(tabs) = pick_pos("where are the session tabs?", cfg.tabs) else {
        crate::config::save_global(cfg);
        return;
    };
    cfg.tabs = tabs;
    crate::config::save_global(cfg);
    crate::config::apply_all_layouts();
}

/// Settings from Ctrl-b: global default or current group, then one bar position.
pub fn cmd_settings(group: &str, client: Option<&str>) {
    let scope_labels = [
        "global default",
        "current group only",
    ];
    let Some(scope) = pick_list("configure", &scope_labels, 0) else {
        return;
    };
    let global = scope == 0;
    let mut cfg = if global {
        crate::config::load_global()
    } else {
        crate::config::resolved(Some(group))
    };
    let which = pick_list(
        "which setting?",
        &["theme", "Ctrl+B / events bar", "session tabs"],
        0,
    );
    let Some(which) = which else {
        return;
    };
    if which == 0 {
        let Some(theme) = pick_theme(&cfg.theme) else {
            return;
        };
        cfg.theme = theme;
    } else {
        let cur = if which == 1 { cfg.events } else { cfg.tabs };
        let title = if which == 1 {
            "Ctrl+B / events bar position"
        } else {
            "session tabs position"
        };
        let Some(pos) = pick_pos(title, cur) else {
            return;
        };
        if which == 1 {
            cfg.events = pos;
        } else {
            cfg.tabs = pos;
        }
    }
    if global {
        crate::config::save_global(cfg);
        if crate::config::group_has_override(group) {
            let ans = pick_list(
                "there is override for this group, remove reset?",
                &["Yes", "No"],
                1,
            );
            if ans == Some(0) {
                crate::config::remove_group_override(group);
            }
        }
        crate::config::apply_all_layouts();
    } else {
        crate::config::save_group_override(group, cfg);
        crate::config::apply_all_layouts();
    }
    start_flash("saved bar layout", client);
}

fn pad_line(text: &str, cols: usize) -> String {
    let n = text.chars().count();
    if n >= cols {
        text.chars().take(cols).collect()
    } else {
        format!("{text}{}", " ".repeat(cols - n))
    }
}

fn draw_reorder(names: &[String], moving: usize) {
    let (cols, rows) = term_size();
    let footer = "j/k or arrows  move · enter  confirm · esc  cancel";
    let header_rows = 3;
    let footer_rows = 2;
    let vis = rows.saturating_sub(header_rows + footer_rows).max(1);
    let start = if moving >= vis { moving + 1 - vis } else { 0 };
    let mut out = String::from("\x1b[H\x1b[J");
    out.push_str("\x1b[38;5;216;1mtabmux\x1b[0m  \x1b[38;5;252mreorder\x1b[0m\n\n");
    for (i, name) in names.iter().enumerate().skip(start).take(vis) {
        let mark = if i == moving { "▸" } else { " " };
        let num = i + 1;
        let body = format!("  {mark} {num:>2}  {name}");
        let line = if i == moving {
            // same blue as the active tab
            format!("\x1b[48;5;25;38;5;231;1m{}\x1b[0m", pad_line(&body, cols))
        } else {
            format!("\x1b[38;5;252m{}\x1b[0m", pad_line(&body, cols))
        };
        out.push_str(&line);
        out.push('\n');
    }
    let r_footer = rows.max(footer_rows);
    out.push_str(&format!("\x1b[{r_footer};1H\x1b[38;5;245m{footer}\x1b[0m"));
    let _ = io::stdout().write_all(out.as_bytes());
    let _ = io::stdout().flush();
}

/// Interactive reorder of `session`. Returns true if the new order was kept.
fn cmd_reorder(session: &str, client: Option<&str>, group: &str) -> bool {
    let snapshot = members_for(session);
    if snapshot.len() < 2 {
        return false;
    }
    let Some(mut idx) = snapshot.iter().position(|s| s == session) else {
        return false;
    };
    let mut names = snapshot.clone();
    let mut dirty = false;
    loop {
        draw_reorder(&names, idx);
        match read_key() {
            Key::Char('j') | Key::Char('J') | Key::Down => {
                if idx + 1 < names.len() {
                    names.swap(idx, idx + 1);
                    idx += 1;
                    set_member_order(group, &names);
                    refresh_bar(client);
                    dirty = true;
                }
            }
            Key::Char('k') | Key::Char('K') | Key::Up => {
                if idx > 0 {
                    names.swap(idx, idx - 1);
                    idx -= 1;
                    set_member_order(group, &names);
                    refresh_bar(client);
                    dirty = true;
                }
            }
            Key::Enter => {
                set_member_order(group, &names);
                refresh_bar(client);
                return true;
            }
            Key::Esc | Key::Char('q') => {
                if dirty {
                    set_member_order(group, &snapshot);
                    refresh_bar(client);
                }
                return false;
            }
            _ => {}
        }
    }
}

fn ansi_fg(tmux_color: &str) -> String {
    let c = tmux_color.trim();
    if let Some(n) = c.strip_prefix("colour") {
        return format!("\x1b[38;5;{n}m");
    }
    if c.starts_with('#') && c.len() >= 7 {
        let r = u8::from_str_radix(&c[1..3], 16).unwrap_or(180);
        let g = u8::from_str_radix(&c[3..5], 16).unwrap_or(180);
        let b = u8::from_str_radix(&c[5..7], 16).unwrap_or(180);
        return format!("\x1b[38;2;{r};{g};{b}m");
    }
    "\x1b[38;5;252m".into()
}

fn draw_status_pick(choices: &[String], idx: usize, session: &str, theme: &crate::theme::Theme) {
    let (cols, rows) = term_size();
    let footer = "j/k or arrows  select · enter  set · esc  cancel";
    let header_rows = 3;
    let footer_rows = 2;
    let vis = rows.saturating_sub(header_rows + footer_rows).max(1);
    let start = if idx >= vis { idx + 1 - vis } else { 0 };
    let mut out = String::from("\x1b[H\x1b[J");
    out.push_str(&format!(
        "\x1b[38;5;216;1mtabmux\x1b[0m  \x1b[38;5;252mstatus  {session}\x1b[0m\n\n"
    ));
    for (i, name) in choices.iter().enumerate().skip(start).take(vis) {
        let mark = if i == idx { "▸" } else { " " };
        let glyph = if name.as_str() == "unset" { "○" } else { "●" };
        let key = match name.as_str() {
            "unset" => "u",
            "idle" => "i",
            "attention" => "a",
            "busy" => "b",
            _ => "",
        };
        let label = if key.is_empty() {
            format!("  {mark}  {glyph}  {name}")
        } else {
            format!("  {mark}  {glyph}  {name:<12} {key}")
        };
        let body = pad_line(&label, cols);
        let painted = if name.as_str() == "unset" {
            format!("\x1b[38;5;245m{body}\x1b[0m")
        } else {
            let fg = ansi_fg(&status_fg(name, theme));
            format!(
                "\x1b[38;5;252m{}\x1b[0m",
                body.replacen(glyph, &format!("{fg}{glyph}\x1b[38;5;252m"), 1)
            )
        };
        let line = if i == idx {
            format!("\x1b[1m{painted}\x1b[0m")
        } else {
            painted
        };
        out.push_str(&line);
        out.push('\n');
    }
    let r_footer = rows.max(footer_rows);
    out.push_str(&format!("\x1b[{r_footer};1H\x1b[38;5;245m{footer}\x1b[0m"));
    let _ = io::stdout().write_all(out.as_bytes());
    let _ = io::stdout().flush();
}

fn cmd_status_pick(session: &str, client: Option<&str>) -> Option<String> {
    let choices = status_choices();
    if choices.is_empty() {
        return None;
    }
    let current = read_status(session).unwrap_or_else(|| "unset".into());
    let mut idx = choices.iter().position(|s| s == &current).unwrap_or(0);
    let theme = crate::config::resolved_for_session(session).theme();
    loop {
        draw_status_pick(&choices, idx, session, &theme);
        match read_key() {
            Key::Char('j') | Key::Char('J') | Key::Down => {
                if idx + 1 < choices.len() {
                    idx += 1;
                }
            }
            Key::Char('k') | Key::Char('K') | Key::Up => {
                if idx > 0 {
                    idx -= 1;
                }
            }
            Key::Enter => {
                let state = choices[idx].clone();
                apply_status(&state, session, client);
                return Some(state);
            }
            Key::Char('u') | Key::Char('U') => {
                apply_status("unset", session, client);
                return Some("unset".into());
            }
            Key::Char('i') | Key::Char('I') => {
                apply_status("idle", session, client);
                return Some("idle".into());
            }
            Key::Char('a') | Key::Char('A') => {
                apply_status("attention", session, client);
                return Some("attention".into());
            }
            Key::Char('b') | Key::Char('B') => {
                apply_status("busy", session, client);
                return Some("busy".into());
            }
            Key::Esc | Key::Char('q') => return None,
            _ => {}
        }
    }
}

pub fn cmd_menu(client: Option<&str>) {
    let client = client.filter(|s| !s.is_empty());
    if unsafe { libc::isatty(0) } == 0 {
        popup_menu(client);
        return;
    }
    let current = client_session(client);
    let group = group_of_session(&current).unwrap_or_else(|| DEFAULT_GROUP.to_string());
    let names = members_for(&current);
    let first = [
        ("q", "close this menu"),
        ("n", "create a new session"),
        ("c", "configs"),
        ("d", "detach (tabmux keeps running)"),
    ];
    let session_ops = [
        ("x", "close the current session"),
        ("r", "rename the current session"),
        ("m", "reorder the current session"),
        ("s", "set status dot"),
    ];
    let mut switches: Vec<(String, String)> = Vec::new();
    for (i, name) in names.iter().enumerate().take(9) {
        let mark = if name == &current { " (current)" } else { "" };
        switches.push((
            (i + 1).to_string(),
            format!("switch to session {}: {name}{mark}", i + 1),
        ));
    }
    switches.push(("h [".into(), "switch to the previous session".into()));
    switches.push(("l ]".into(), "switch to the next session".into()));
    switches.push(("p".into(), "switch to the last session you were on".into()));
    switches.push((";".into(), "switch to the next idle session".into()));

    let mut left: Vec<(String, String)> = first
        .iter()
        .map(|(k, d)| ((*k).to_string(), (*d).to_string()))
        .collect();
    left.push((String::new(), String::new()));
    left.extend(
        session_ops
            .iter()
            .map(|(k, d)| ((*k).to_string(), (*d).to_string())),
    );

    let (cols, rows) = term_size();
    let (table, tw) = render_menu_table(&left, &switches, cols);
    let table_lines: Vec<&str> = table.lines().collect();
    let title = "TabMux";
    let footer = "press a key";
    let block_h = 2 + table_lines.len() + 2;
    let r0 = 1.max(rows.saturating_sub(block_h) / 2 + 1);
    let c0 = 1.max(cols.saturating_sub(tw) / 2 + 1);
    let title_c = c0 + tw.saturating_sub(title.len()) / 2;
    let footer_c = c0 + tw.saturating_sub(footer.len()) / 2;
    let mut out = String::from("\x1b[H\x1b[J");
    out.push_str(&format!("\x1b[{r0};{title_c}H\x1b[38;5;216;1m{title}\x1b[0m"));
    for (i, line) in table_lines.iter().enumerate() {
        out.push_str(&format!("\x1b[{};{c0}H{line}", r0 + 2 + i));
    }
    let footer_r = r0 + 2 + table_lines.len() + 1;
    out.push_str(&format!("\x1b[{footer_r};{footer_c}H\x1b[38;5;245m{footer}\x1b[0m"));
    let _ = io::stdout().write_all(out.as_bytes());
    let _ = io::stdout().flush();

    let ch = read_raw().map(|b| b as char).unwrap_or('\0');
    match ch {
        'q' | '\r' | '\n' | '\u{1b}' => {}
        'n' => {
            let (new_name, created) = cmd_new(&prompt_new_session_name(&group), client, Some(&group));
            start_flash(
                &if created {
                    format!("created {new_name}")
                } else {
                    format!("switched to {new_name}")
                },
                client,
            );
        }
        'x' | 'X' => {
            let sess = if current.is_empty() {
                client_session(client)
            } else {
                current
            };
            if sess.is_empty() {
                start_flash("nothing to close", client);
            } else {
                match close_session(&sess, client) {
                    CloseOutcome::SwitchedTo(next) => {
                        start_flash(&format!("closed {sess}, switched to {next}"), client)
                    }
                    CloseOutcome::Quit => {}
                    CloseOutcome::Failed => start_flash("nothing to close", client),
                }
            }
        }
        'r' => {
            let sess = if current.is_empty() {
                client_session(client)
            } else {
                current
            };
            if sess.is_empty() {
                start_flash("unknown action (r)", client);
            } else {
                let new_name = prompt_text("rename session", &format!("empty keeps {sess}"));
                stamp_group_ids(&group);
                match cmd_rename(&sess, &new_name, client) {
                    Ok(()) if new_name.trim().is_empty() || new_name.trim() == sess => {}
                    Ok(()) => {
                        rename_member(&group, &sess, new_name.trim());
                        start_flash(&format!("renamed {sess} to {}", new_name.trim()), client);
                    }
                    Err(e) => start_flash(&e, client),
                }
            }
        }
        'm' => {
            let sess = if current.is_empty() {
                client_session(client)
            } else {
                current
            };
            if sess.is_empty() || names.len() < 2 {
                start_flash("nothing to reorder", client);
            } else if cmd_reorder(&sess, client, &group) {
                start_flash("reordered", client);
            }
        }
        's' => {
            let sess = if current.is_empty() {
                client_session(client)
            } else {
                current
            };
            if sess.is_empty() {
                start_flash("nothing to set", client);
            } else if let Some(state) = cmd_status_pick(&sess, client) {
                start_flash(
                    &if state == "unset" {
                        format!("cleared status on {sess}")
                    } else {
                        format!("{sess} {state}")
                    },
                    client,
                );
            }
        }
        '1'..='9' => {
            let idx = ch.to_digit(10).unwrap() as usize;
            cmd_nth(&names, idx, client);
            if idx >= 1 && idx <= names.len() {
                start_flash(&format!("switched to {}", names[idx - 1]), client);
            } else {
                start_flash(&format!("unknown action ({ch})"), client);
            }
        }
        '[' | 'h' => {
            if let Some(to) = cmd_step(&current, -1, client) {
                start_flash(&format!("switched to {to}"), client);
            }
        }
        ']' | 'l' => {
            if let Some(to) = cmd_step(&current, 1, client) {
                start_flash(&format!("switched to {to}"), client);
            }
        }
        ';' => {
            match cmd_step_status(&current, "idle", client) {
                Some(to) => start_flash(&format!("switched to {to}"), client),
                None => start_flash("no idle session", client),
            }
        }
        'p' => {
            if let Some(c) = client {
                tmux(&["switch-client", "-c", c, "-p"]);
            } else {
                tmux(&["switch-client", "-p"]);
            }
            start_flash(&format!("switched to {}", client_session(client)), client);
        }
        'c' => {
            cmd_settings(&group, client);
        }
        'd' => {
            if let Some(c) = client {
                tmux(&["detach-client", "-t", c]);
            } else {
                tmux(&["detach-client"]);
            }
        }
        '\0' => {}
        _ => start_flash(&format!("unknown action ({ch})"), client),
    }
}
