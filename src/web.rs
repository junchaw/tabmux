use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use serde_json::{json, Value};

use crate::actions::{
    apply_status, cmd_new, cmd_rename, cmd_step, cmd_step_status, fmt_age, load_messages,
    read_status, start_flash, status_choices, status_fg,
};
use crate::config::{
    apply_all_layouts, group_has_override, load_global, remove_group_override, resolved, save_global,
    save_group_override, Pos,
};
use crate::groups::{
    close_session, group_members, group_of_session, list_groups, rename_member, set_member_order,
    CloseOutcome, DEFAULT_GROUP,
};
use crate::theme::list_theme_ids;
use crate::tmux::{list_session_rows, switch_to, tmux, tmux_stdout};

const PAGE: &str = include_str!("../assets/web.html");

pub fn serve(args: &[String]) {
    let host = args.first().map(|s| s.as_str()).unwrap_or("127.0.0.1");
    let port: u16 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(8791);
    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("tabmux web: {addr}: {e}");
        std::process::exit(1);
    });
    println!("tabmux web http://{addr}");
    for conn in listener.incoming() {
        let Ok(mut stream) = conn else { continue };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(8)));
        if let Some((method, path, body, head)) = read_request(&mut stream) {
            let (only, query) = path.split_once('?').unwrap_or((path.as_str(), ""));
            if method == "GET" && only == "/api/tty" {
                if let Some(key) = header_value(&head, "sec-websocket-key") {
                    let focus = query_param(query, "focus");
                    std::thread::spawn(move || {
                        if let Err(e) = tty_connection(stream, &key, focus) {
                            eprintln!("tabmux tty: {e}");
                        }
                    });
                    continue;
                }
            }
            let (status, ctype, payload) = route(&method, &path, &body);
            let head = format!(
                "HTTP/1.1 {status}\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\nconnection: close\r\ncache-control: no-store\r\n\r\n",
                payload.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&payload);
        }
    }
}

fn route(method: &str, full: &str, body: &[u8]) -> (u16, &'static str, Vec<u8>) {
    let (path, query) = full.split_once('?').unwrap_or((full, ""));
    let focus = query_param(query, "focus");
    match (method, path) {
        ("GET", "/") => (200, "text/html; charset=utf-8", PAGE.as_bytes().to_vec()),
        ("GET", "/api/state") => json_ok(snapshot(&focus)),
        ("POST", "/api/switch") => {
            let session = field(&parse_json(body), "session");
            if session.is_empty() {
                return json_err(400, "missing session");
            }
            if let Err(e) = require_client() {
                return json_err(409, &e);
            }
            switch_to(&session, None);
            start_flash(&format!("switched to {session}"), None);
            json_ok(json!({ "ok": true, "session": session }))
        }
        ("POST", "/api/status") => {
            let v = parse_json(body);
            let session = field(&v, "session");
            let state = field(&v, "state");
            if session.is_empty() || state.is_empty() {
                return json_err(400, "missing session or state");
            }
            apply_status(&state, &session, None);
            let msg = if state == "unset" {
                format!("cleared status on {session}")
            } else {
                format!("{session} {state}")
            };
            start_flash(&msg, None);
            json_ok(json!({ "ok": true }))
        }
        ("POST", "/api/new") => {
            let v = parse_json(body);
            let group = field(&v, "group");
            let name = field(&v, "name");
            let group_arg = if group.is_empty() { None } else { Some(group.as_str()) };
            let (created_name, created) = cmd_new(&name, None, group_arg);
            if created {
                start_flash(&format!("created {created_name}"), None);
            }
            json_ok(json!({ "ok": true, "session": created_name, "created": created }))
        }
        ("POST", "/api/next-idle") => step_status(&field(&parse_json(body), "session")),
        ("POST", "/api/step") => {
            let v = parse_json(body);
            let session = field(&v, "session");
            let delta = v.get("delta").and_then(|n| n.as_i64()).unwrap_or(0) as i32;
            if let Err(e) = require_client() {
                return json_err(409, &e);
            }
            match cmd_step(&session, delta, None) {
                Some(to) => {
                    start_flash(&format!("switched to {to}"), None);
                    json_ok(json!({ "ok": true, "session": to }))
                }
                None => json_err(404, "nothing to switch"),
            }
        }
        ("POST", "/api/last") => {
            if let Err(e) = require_client() {
                return json_err(409, &e);
            }
            tmux(&["switch-client", "-p"]);
            let to = tmux_stdout(&["display-message", "-p", "#{session_name}"]);
            let to = to.trim();
            start_flash(&format!("switched to {to}"), None);
            json_ok(json!({ "ok": true, "session": to }))
        }
        ("POST", "/api/close") => {
            let session = field(&parse_json(body), "session");
            match close_session(&session, None) {
                CloseOutcome::SwitchedTo(next) => {
                    start_flash(&format!("closed {session}, switched to {next}"), None);
                    json_ok(json!({ "ok": true, "session": next }))
                }
                CloseOutcome::Quit => json_ok(json!({ "ok": true, "quit": true })),
                CloseOutcome::Failed => json_err(400, "nothing to close"),
            }
        }
        ("POST", "/api/rename") => {
            let v = parse_json(body);
            let session = field(&v, "session");
            let name = field(&v, "name");
            if name.is_empty() || name == session {
                return json_ok(json!({ "ok": true }));
            }
            let group = group_of_session(&session).unwrap_or_else(|| DEFAULT_GROUP.to_string());
            match cmd_rename(&session, &name, None) {
                Ok(()) => {
                    rename_member(&group, &session, &name);
                    start_flash(&format!("renamed {session} to {name}"), None);
                    json_ok(json!({ "ok": true, "session": name }))
                }
                Err(e) => json_err(400, &e),
            }
        }
        ("POST", "/api/order") => {
            let v = parse_json(body);
            let group = field(&v, "group");
            let names = v
                .get("names")
                .and_then(|n| n.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if group.is_empty() || names.len() < 2 {
                return json_err(400, "nothing to reorder");
            }
            set_member_order(&group, &names);
            json_ok(json!({ "ok": true }))
        }
        ("POST", "/api/detach") => {
            tmux(&["detach-client"]);
            json_ok(json!({ "ok": true }))
        }
        ("POST", "/api/layout") => save_layout(&parse_json(body)),
        ("POST", "/api/keys") => send_input(&parse_json(body)),
        _ => (404, "text/plain; charset=utf-8", b"not found".to_vec()),
    }
}

fn step_status(session: &str) -> (u16, &'static str, Vec<u8>) {
    if let Err(e) = require_client() {
        return json_err(409, &e);
    }
    match cmd_step_status(session, "idle", None) {
        Some(to) => {
            start_flash(&format!("switched to {to}"), None);
            json_ok(json!({ "ok": true, "session": to }))
        }
        None => {
            start_flash("no idle session", None);
            json_err(404, "no idle session")
        }
    }
}

fn save_layout(v: &Value) -> (u16, &'static str, Vec<u8>) {
    let group = field(v, "group");
    let scope = field(v, "scope");
    let global = scope != "group";
    let mut cfg = if global {
        load_global()
    } else {
        resolved(Some(&group))
    };
    if let Some(p) = pos(field(v, "events").as_str()) {
        cfg.events = p;
    }
    if let Some(p) = pos(field(v, "tabs").as_str()) {
        cfg.tabs = p;
    }
    let theme = field(v, "theme");
    if !theme.is_empty() {
        cfg.theme = theme;
    }
    if global {
        save_global(cfg);
        if v.get("clearOverride").and_then(|x| x.as_bool()) == Some(true) {
            remove_group_override(&group);
        }
    } else {
        save_group_override(&group, cfg);
    }
    apply_all_layouts();
    start_flash("saved bar layout", None);
    json_ok(json!({ "ok": true, "override": group_has_override(&group) }))
}

fn session_target(session: &str) -> Result<String, String> {
    if session.is_empty() {
        return Err("missing session".into());
    }
    list_session_rows()
        .into_iter()
        .find(|s| s.name == session)
        .map(|s| s.id)
        .ok_or_else(|| format!("no session {session}"))
}

struct PaneShot {
    text: String,
    x: u32,
    y: u32,
    cols: u32,
    rows: u32,
}

fn capture_pane(session: &str) -> PaneShot {
    let Ok(target) = session_target(session) else {
        return PaneShot { text: String::new(), x: 0, y: 0, cols: 0, rows: 0 };
    };
    let meta = tmux_stdout(&[
        "display-message",
        "-p",
        "-t",
        &target,
        "#{cursor_x} #{cursor_y} #{pane_width} #{pane_height}",
    ]);
    let mut nums = meta.split_whitespace().filter_map(|s| s.parse::<u32>().ok());
    let x = nums.next().unwrap_or(0);
    let y = nums.next().unwrap_or(0);
    let cols = nums.next().unwrap_or(0);
    let rows = nums.next().unwrap_or(0);
    let out = tmux(&["capture-pane", "-p", "-t", &target]);
    PaneShot {
        text: String::from_utf8_lossy(&out.stdout).into_owned(),
        x,
        y,
        cols,
        rows,
    }
}

fn send_input(v: &Value) -> (u16, &'static str, Vec<u8>) {
    let session = field(v, "session");
    let target = match session_target(&session) {
        Ok(t) => t,
        Err(e) => return json_err(404, &e),
    };
    if let Some(text) = v.get("literal").and_then(|x| x.as_str()) {
        if text.is_empty() {
            return json_ok(json!({ "ok": true }));
        }
        let out = tmux(&["send-keys", "-t", &target, "-l", "--", text]);
        return tmux_result(out);
    }
    let key = field(v, "key");
    if !allowed_key(&key) {
        return json_err(400, "unsupported key");
    }
    tmux_result(tmux(&["send-keys", "-t", &target, &key]))
}

fn allowed_key(key: &str) -> bool {
    matches!(
        key,
        "Enter"
            | "Escape"
            | "Tab"
            | "BSpace"
            | "DC"
            | "Space"
            | "Up"
            | "Down"
            | "Left"
            | "Right"
            | "Home"
            | "End"
            | "PPage"
            | "NPage"
            | "IC"
    ) || (key.len() == 3
        && (key.starts_with("C-") || key.starts_with("M-"))
        && key.as_bytes()[2].is_ascii_alphanumeric())
}

fn tmux_result(out: std::process::Output) -> (u16, &'static str, Vec<u8>) {
    if out.status.success() {
        json_ok(json!({ "ok": true }))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        json_err(500, err.trim())
    }
}

fn query_param(query: &str, key: &str) -> String {
    for pair in query.split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        if k == key {
            return url_decode(v);
        }
    }
    String::new()
}

fn url_decode(raw: &str) -> String {
    let raw = raw.replace('+', " ");
    let bytes = raw.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn snapshot(focus: &str) -> Value {
    let attached: Vec<String> = tmux_stdout(&["list-clients", "-F", "#{session_name}"])
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let current = attached.first().cloned().unwrap_or_default();
    let anchor = if !focus.is_empty() {
        focus.to_string()
    } else {
        current.clone()
    };
    let group = if anchor.is_empty() {
        list_groups()
            .into_iter()
            .next()
            .unwrap_or_else(|| DEFAULT_GROUP.to_string())
    } else {
        group_of_session(&anchor).unwrap_or_else(|| DEFAULT_GROUP.to_string())
    };
    let cfg = resolved(Some(&group));
    let th = cfg.theme();
    let sessions = group_members(&group)
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let status = read_status(&name);
            let color = status
                .as_deref()
                .map(|s| css_color(&status_fg(s, &th)));
            json!({
                "name": name,
                "index": i + 1,
                "status": status,
                "color": color,
                "attached": attached.iter().any(|s| s == &name),
            })
        })
        .collect::<Vec<_>>();
    let messages = load_messages(&group)
        .into_iter()
        .map(|(ts, text)| json!({ "age": fmt_age(ts), "text": text }))
        .collect::<Vec<_>>();
    let pane_session = if !anchor.is_empty() {
        anchor.clone()
    } else {
        sessions
            .first()
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string()
    };
    let pane = capture_pane(&pane_session).text;
    json!({
        "group": group,
        "current": current,
        "themeId": cfg.theme,
        "hasClient": !attached.is_empty(),
        "override": group_has_override(&group),
        "layout": { "events": cfg.events.as_str(), "tabs": cfg.tabs.as_str() },
        "theme": theme_json(&th),
        "themes": list_theme_ids(),
        "statuses": status_choices(),
        "sessions": sessions,
        "messages": messages,
        "paneSession": pane_session,
        "pane": pane,
    })
}

fn theme_json(th: &crate::theme::Theme) -> Value {
    json!({
        "activeBg": css_color(&th.active_bg),
        "activeFg": css_color(&th.active_fg),
        "inactiveBg": css_color(&th.inactive_bg),
        "inactiveFg": css_color(&th.inactive_fg),
        "hintBg": css_color(&th.hint_bg),
        "hintFg": css_color(&th.hint_fg),
        "msgBg": css_color(&th.msg_bg),
        "msgFg": css_color(&th.msg_fg),
    })
}

fn pos(s: &str) -> Option<Pos> {
    match s {
        "top" => Some(Pos::Top),
        "bottom" => Some(Pos::Bottom),
        _ => None,
    }
}

fn require_client() -> Result<(), String> {
    if tmux_stdout(&["list-clients"]).trim().is_empty() {
        Err("no attached tabmux client".into())
    } else {
        Ok(())
    }
}

fn css_color(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(hex) = raw.strip_prefix('#') {
        if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return format!("#{hex}");
        }
    }
    let n = raw
        .strip_prefix("colour")
        .or_else(|| raw.strip_prefix("color"))
        .and_then(|s| s.parse::<u16>().ok());
    let Some(i) = n else {
        return "#d8dee9".into();
    };
    ansi256(i.min(255) as u8)
}

fn ansi256(i: u8) -> String {
    const BASIC: [&str; 16] = [
        "#000000", "#800000", "#008000", "#808000", "#000080", "#800080", "#008080", "#c0c0c0",
        "#808080", "#ff0000", "#00ff00", "#ffff00", "#0000ff", "#ff00ff", "#00ffff", "#ffffff",
    ];
    if i < 16 {
        return BASIC[i as usize].into();
    }
    if i >= 232 {
        let v = 8 + (i - 232) * 10;
        return format!("#{v:02x}{v:02x}{v:02x}");
    }
    let n = i - 16;
    let level = |c: u8| if c == 0 { 0 } else { 55 + 40 * c };
    let r = level(n / 36);
    let g = level((n % 36) / 6);
    let b = level(n % 6);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn parse_json(body: &[u8]) -> Value {
    serde_json::from_slice(body).unwrap_or(Value::Null)
}

fn field(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").trim().to_string()
}

fn json_ok(v: Value) -> (u16, &'static str, Vec<u8>) {
    (200, "application/json", v.to_string().into_bytes())
}

fn json_err(code: u16, msg: &str) -> (u16, &'static str, Vec<u8>) {
    (
        code,
        "application/json",
        json!({ "error": msg }).to_string().into_bytes(),
    )
}

fn header_value(head: &str, name: &str) -> Option<String> {
    for line in head.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        if k.eq_ignore_ascii_case(name) {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn read_request(stream: &mut impl Read) -> Option<(String, String, Vec<u8>, String)> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 1024 * 1024 {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();
    let request = lines.next().unwrap_or("");
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut len = 0usize;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else { continue };
        if k.eq_ignore_ascii_case("content-length") {
            len = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(len);
    Some((method, path, body, head))
}

fn tty_connection(mut stream: TcpStream, key: &str, mut session: String) -> std::io::Result<()> {
    let accept = b64(&sha1(
        format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
    ));
    let handshake = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(handshake.as_bytes())?;
    stream.set_read_timeout(Some(Duration::from_millis(60)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut last = String::new();
    loop {
        if session.is_empty() {
            session = tmux_stdout(&["list-clients", "-F", "#{session_name}"])
                .lines()
                .find_map(|l| {
                    let s = l.trim();
                    if s.is_empty() { None } else { Some(s.to_string()) }
                })
                .unwrap_or_default();
        }
        match read_ws_frame(&mut stream) {
            Ok(Some(payload)) => {
                if apply_tty_message(&payload, &mut session) {
                    last.clear();
                }
            }
            Ok(None) => {}
            Err(_) => break,
        }
        if session.is_empty() {
            continue;
        }
        let shot = capture_pane(&session);
        let sig = format!("{}\n{}\n{}", shot.x, shot.y, shot.text);
        if sig != last {
            last = sig;
            let msg = json!({
                "type": "pane",
                "session": session,
                "text": shot.text,
                "x": shot.x,
                "y": shot.y,
                "cols": shot.cols,
                "rows": shot.rows
            }).to_string();
            if write_ws_text(&mut stream, &msg).is_err() {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    Ok(())
}

fn apply_tty_message(payload: &[u8], session: &mut String) -> bool {
    let v = parse_json(payload);
    let kind = field(&v, "type");
    let named = field(&v, "session");
    if !named.is_empty() {
        *session = named;
    }
    match kind.as_str() {
        "watch" => true,
        "literal" => {
            let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
            if !text.is_empty() {
                if let Ok(target) = session_target(session) {
                    let _ = tmux(&["send-keys", "-t", &target, "-l", "--", text]);
                }
            }
            false
        }
        "key" => {
            let key = field(&v, "key");
            if allowed_key(&key) {
                if let Ok(target) = session_target(session) {
                    let _ = tmux(&["send-keys", "-t", &target, &key]);
                }
            }
            false
        }
        _ => false,
    }
}

fn read_ws_frame(stream: &mut TcpStream) -> std::io::Result<Option<Vec<u8>>> {
    let mut hdr = [0u8; 2];
    if let Err(e) = read_full(stream, &mut hdr, true) {
        return if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock {
            Ok(None)
        } else {
            Err(e)
        };
    }
    let opcode = hdr[0] & 0x0f;
    let masked = hdr[1] & 0x80 != 0;
    let mut len = (hdr[1] & 0x7f) as u64;
    if len == 126 {
        let mut ext = [0u8; 2];
        read_full(stream, &mut ext, false)?;
        len = u16::from_be_bytes(ext) as u64;
    } else if len == 127 {
        let mut ext = [0u8; 8];
        read_full(stream, &mut ext, false)?;
        len = u64::from_be_bytes(ext);
    }
    if len > 1_000_000 {
        return Err(std::io::Error::new(ErrorKind::InvalidData, "frame too large"));
    }
    let mut mask = [0u8; 4];
    if masked {
        read_full(stream, &mut mask, false)?;
    }
    let mut payload = vec![0u8; len as usize];
    if len > 0 {
        read_full(stream, &mut payload, false)?;
    }
    if masked {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
    }
    match opcode {
        0x8 => Err(std::io::Error::new(ErrorKind::ConnectionAborted, "close")),
        0x9 => {
            write_ws_frame(stream, 0xA, &payload)?;
            Ok(None)
        }
        0x1 | 0x2 => Ok(Some(payload)),
        _ => Ok(None),
    }
}

fn read_full(stream: &mut TcpStream, buf: &mut [u8], first: bool) -> std::io::Result<()> {
    let mut got = 0;
    while got < buf.len() {
        match stream.read(&mut buf[got..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "closed",
                ))
            }
            Ok(n) => got += n,
            Err(e)
                if first
                    && got == 0
                    && (e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock) =>
            {
                return Err(e)
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn write_ws_text(stream: &mut TcpStream, text: &str) -> std::io::Result<()> {
    write_ws_frame(stream, 0x1, text.as_bytes())
}

fn write_ws_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut hdr = vec![0x80 | opcode];
    let n = payload.len();
    if n < 126 {
        hdr.push(n as u8);
    } else if n <= u16::MAX as usize {
        hdr.push(126);
        hdr.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        hdr.push(127);
        hdr.extend_from_slice(&(n as u64).to_be_bytes());
    }
    stream.write_all(&hdr)?;
    stream.write_all(payload)?;
    stream.flush()?;
    Ok(())
}

fn sha1(message: &[u8]) -> [u8; 20] {
    let mut data = message.to_vec();
    let bits = (message.len() as u64).wrapping_mul(8);
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bits.to_be_bytes());
    let mut h = [
        0x67452301u32,
        0xEFCDAB89,
        0x98BADCFE,
        0x10325476,
        0xC3D2E1F0,
    ];
    for chunk in data.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    if i < data.len() {
        let mut n = (data[i] as u32) << 16;
        out.push(T[((n >> 18) & 63) as usize] as char);
        if i + 1 < data.len() {
            n |= (data[i + 1] as u32) << 8;
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push(T[((n >> 6) & 63) as usize] as char);
            out.push('=');
        } else {
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        }
    }
    out
}
