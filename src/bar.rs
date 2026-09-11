use crate::actions::{fmt_age, load_messages, read_status, start_flash, status_fg};
use crate::groups::{close_session, group_of_session, members_for, CloseOutcome, DEFAULT_GROUP};
use crate::tmux::{sty, switch_to, HINT, MSG_SEP};

const STATUS_DOT: &str = "\u{25cf}";

fn display_name(name: &str) -> String {
    if let Some(pos) = name.rfind("-claude") {
        let rest = &name[pos + 7..];
        if rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit()) {
            return name[..pos].to_string();
        }
    }
    name.to_string()
}

fn vis_len(s: &str) -> usize {
    s.chars().count()
}

fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn take_last_chars(s: &str, n: usize) -> String {
    let total = vis_len(s);
    if n >= total {
        return s.to_string();
    }
    s.chars().skip(total - n).collect()
}

fn fit(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let n = vis_len(text);
    if n > width {
        if width <= 2 {
            return take_chars(text, width);
        }
        return format!("{}…{}", take_chars(text, width - 2), take_last_chars(text, 1));
    }
    let left = (width - n) / 2;
    let right = width - n - left;
    format!("{}{}{}", "\u{00a0}".repeat(left), text, "\u{00a0}".repeat(right))
}

fn label_for(index: usize, name: &str, width: usize, status: Option<&str>) -> String {
    if width <= 3 {
        return fit(&(index + 1).to_string(), width);
    }
    let body = format!("{} ({})", display_name(name), index + 1);
    if status.is_some() && width > 4 {
        fit(&format!("{STATUS_DOT} {body}"), width)
    } else {
        fit(&body, width)
    }
}

fn layout(total: usize, current: &str) -> (Vec<String>, Vec<usize>, Vec<usize>) {
    let names = members_for(current);
    let n = names.len();
    if n == 0 || total == 0 {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    let mut cell = total / n;
    if cell < 1 {
        cell = 1;
    }
    let rem = total % n;
    let mut starts = Vec::new();
    let mut widths = Vec::new();
    let mut x = 0;
    for i in 0..n {
        let w = cell + if i < rem { 1 } else { 0 };
        starts.push(x);
        widths.push(w);
        x += w;
    }
    (names, starts, widths)
}

fn hit(x: usize, total: usize, current: &str) -> Option<String> {
    let (names, starts, widths) = layout(total, current);
    for i in 0..names.len() {
        let a = starts[i];
        let b = a + widths[i];
        if x >= a && x < b {
            return Some(names[i].clone());
        }
    }
    None
}

pub fn cmd_render_msgs(width: usize, current: &str) {
    let width = width.max(1);
    let current = current.trim().trim_matches(|c| c == '\'' || c == '"');
    let th = crate::config::resolved_for_session(current).theme();
    let group = group_of_session(current).unwrap_or_else(|| DEFAULT_GROUP.to_string());
    let stream = load_messages(&group)
        .into_iter()
        .map(|(ts, text)| format!("{} {text}", fmt_age(ts)))
        .collect::<Vec<_>>()
        .join(MSG_SEP);
    let left = format!(" {HINT} ");
    let rest = width as isize - vis_len(&left) as isize;
    if rest <= 0 {
        print!("{}", sty(&th.hint_bg, &th.hint_fg, &take_chars(&left, width), true));
        return;
    }
    let rest = rest as usize;
    let right = if stream.is_empty() {
        " ".repeat(rest)
    } else if vis_len(&stream) <= rest {
        format!("{stream:>rest$}")
    } else {
        let ell = "...";
        let take = rest.saturating_sub(vis_len(ell));
        let mut right = format!("{ell}{}", take_last_chars(&stream, take));
        right = take_chars(&right, rest);
        while vis_len(&right) < rest {
            right.push(' ');
        }
        right
    };
    print!(
        "{}{}",
        sty(&th.hint_bg, &th.hint_fg, &left, true),
        sty(&th.msg_bg, &th.msg_fg, &right, false)
    );
}

pub fn cmd_render(width: usize, current: &str) {
    let width = width.saturating_add(1).max(1);
    let current = current.trim().trim_matches(|c| c == '\'' || c == '"');
    let th = crate::config::resolved_for_session(current).theme();
    let (names, _, widths) = layout(width, current);
    if names.is_empty() {
        print!(" ");
        return;
    }
    let mut out = String::new();
    for (i, (name, w)) in names.iter().zip(widths.iter()).enumerate() {
        let (bg, fg, bold) = if name == current {
            (th.active_bg.as_str(), th.active_fg.as_str(), true)
        } else {
            (th.inactive_bg.as_str(), th.inactive_fg.as_str(), false)
        };
        let status = read_status(name);
        let fitted = label_for(i, name, *w, status.as_deref());
        if let Some(state) = status.as_deref() {
            if let Some(pos) = fitted.find(STATUS_DOT) {
                let (before, rest) = fitted.split_at(pos);
                let after = &rest[STATUS_DOT.len()..];
                let dot_fg = status_fg(state, &th);
                out.push_str(&sty(bg, fg, before, bold));
                out.push_str(&sty(bg, &dot_fg, STATUS_DOT, bold));
                out.push_str(&sty(bg, fg, after, bold));
                continue;
            }
        }
        out.push_str(&sty(bg, fg, &fitted, bold));
    }
    let used: usize = widths.iter().sum();
    if used < width {
        out.push_str(&sty(
            &th.inactive_bg,
            &th.inactive_fg,
            &" ".repeat(width - used),
            false,
        ));
    }
    print!("{out}");
}

pub fn cmd_click(x: usize, width: usize, client: Option<&str>, line: i32, current: &str) {
    let cfg = crate::config::resolved_for_session(current);
    if crate::config::status_line_is_events(&cfg, line) {
        crate::menu::popup_menu(client);
        return;
    }
    if let Some(target) = hit(x, width, current) {
        switch_to(&target, client);
        start_flash(&format!("switched to {target}"), client);
    }
}

pub fn cmd_click_close(x: usize, width: usize, client: Option<&str>, line: i32, current: &str) {
    let cfg = crate::config::resolved_for_session(current);
    if crate::config::status_line_is_events(&cfg, line) {
        return;
    }
    let Some(target) = hit(x, width, current) else {
        return;
    };
    match close_session(&target, client) {
        CloseOutcome::SwitchedTo(next) => {
            start_flash(&format!("closed {target}, switched to {next}"), client)
        }
        CloseOutcome::Quit => {}
        CloseOutcome::Failed => start_flash(&format!("couldn't close {target}"), client),
    }
}
