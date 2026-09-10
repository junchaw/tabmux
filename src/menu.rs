use std::io::{self, Write};

use crate::actions::{cmd_new, cmd_nth, cmd_rename, start_flash};
use crate::groups::{
    close_session, group_of_session, members_for, rename_member, set_member_order, stamp_group_ids,
    DEFAULT_GROUP,
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

pub fn cmd_menu(client: Option<&str>) {
    let client = client.filter(|s| !s.is_empty());
    if unsafe { libc::isatty(0) } == 0 {
        popup_menu(client);
        return;
    }
    let current = client_session(client);
    let group = group_of_session(&current).unwrap_or_else(|| DEFAULT_GROUP.to_string());
    let names = members_for(&current);
    let general = [
        ("x", "close the current session"),
        ("q", "close this menu"),
        ("n", "create a new session"),
        ("r", "rename the current session"),
        ("m", "reorder sessions"),
        ("d", "detach (tabmux keeps running)"),
    ];
    let mut switches: Vec<(String, String)> = Vec::new();
    for (i, name) in names.iter().enumerate().take(9) {
        let mark = if name == &current { " (current)" } else { "" };
        switches.push((
            (i + 1).to_string(),
            format!("switch to session {}: {name}{mark}", i + 1),
        ));
    }
    switches.push(("p".into(), "switch to previous session".into()));

    let mut out = String::from("\x1b[H\x1b[Jtabmux\n\n");
    out.push_str("  key       action\n");
    out.push_str("  ---       ------\n");
    for (k, d) in general {
        out.push_str(&format!("  \x1b[1m{k:<8}\x1b[0m  {d}\n"));
    }
    out.push('\n');
    for (k, d) in &switches {
        out.push_str(&format!("  \x1b[1m{k:<8}\x1b[0m  {d}\n"));
    }
    out.push_str("\n  press a key\n");
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
                    Some(next) => start_flash(&format!("closed {sess}, switched to {next}"), client),
                    None => start_flash("nothing to close", client),
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
        '1'..='9' => {
            let idx = ch.to_digit(10).unwrap() as usize;
            cmd_nth(&names, idx, client);
            if idx >= 1 && idx <= names.len() {
                start_flash(&format!("switched to {}", names[idx - 1]), client);
            } else {
                start_flash(&format!("unknown action ({ch})"), client);
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
