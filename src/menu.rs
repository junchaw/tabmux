use std::io::{self, Write};

use crate::actions::{cmd_close, cmd_new, cmd_nth, cmd_rename, start_flash};
use crate::tmux::{launcher, list_sessions, tmux, unique_name};

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

fn read_raw() -> Option<u8> {
    unsafe {
        let fd = 0;
        let mut old: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut old) != 0 {
            return None;
        }
        let mut raw = old;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
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

fn prompt_new_session_name() -> String {
    let default = unique_name();
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

pub fn cmd_menu(client: Option<&str>) {
    let client = client.filter(|s| !s.is_empty());
    if unsafe { libc::isatty(0) } == 0 {
        popup_menu(client);
        return;
    }
    let names = list_sessions();
    let current = client_session(client);
    let general = [
        ("x", "close the current session"),
        ("q", "close this menu"),
        ("n", "create a new session"),
        ("r", "rename the current session"),
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
            let (new_name, created) = cmd_new(&prompt_new_session_name(), client);
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
                start_flash("won't close last session", client);
            } else if cmd_close(&sess, client).is_some() {
                start_flash(&format!("closed {sess}"), client);
            } else {
                start_flash("won't close last session", client);
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
                match cmd_rename(&sess, &new_name, client) {
                    Ok(()) if new_name.trim().is_empty() || new_name.trim() == sess => {}
                    Ok(()) => start_flash(&format!("renamed {sess} to {}", new_name.trim()), client),
                    Err(e) => start_flash(&e, client),
                }
            }
        }
        '1'..='9' => {
            let idx = ch.to_digit(10).unwrap() as usize;
            let names = list_sessions();
            cmd_nth(idx, client);
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
