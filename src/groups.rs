use crate::tmux::{
    ensure_dir, group_path, groups_dir, list_session_rows, status_path, switch_to, tmux, tmux_ok,
    unique_name, Session,
};

pub const DEFAULT_GROUP: &str = "default";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Member {
    /// tmux `#{session_id}` (`$0`, …). Stable across rename. Absent on
    /// legacy name-only group files until the next stamp.
    id: Option<String>,
    name: String,
}

fn server_alive() -> bool {
    tmux_ok(&["list-sessions"])
}

fn parse_member(line: &str) -> Option<Member> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Some((id, name)) = line.split_once('\t') {
        if id.starts_with('$') && !name.is_empty() {
            return Some(Member {
                id: Some(id.to_string()),
                name: name.to_string(),
            });
        }
    }
    Some(Member {
        id: None,
        name: line.to_string(),
    })
}

fn load_raw(group: &str) -> Vec<Member> {
    let Ok(raw) = std::fs::read_to_string(group_path(group)) else {
        return Vec::new();
    };
    raw.lines().filter_map(parse_member).collect()
}

fn write_members(group: &str, members: &[Member]) {
    let path = group_path(group);
    ensure_dir(&path);
    let body: String = members
        .iter()
        .map(|m| match &m.id {
            Some(id) => format!("{id}\t{}\n", m.name),
            None => format!("{}\n", m.name),
        })
        .collect();
    let _ = std::fs::write(&path, body);
}

/// Prefer session id (survives rename); fall back to name for legacy rows.
/// Returns the kept members and whether the on-disk list should be rewritten.
fn resolve_members(members: Vec<Member>, live: &[Session]) -> (Vec<Member>, bool) {
    let mut kept = Vec::new();
    let mut changed = false;
    for mut m in members {
        if let Some(id) = &m.id {
            if let Some(s) = live.iter().find(|s| &s.id == id) {
                if m.name != s.name {
                    m.name = s.name.clone();
                    changed = true;
                }
                kept.push(m);
                continue;
            }
        }
        if let Some(s) = live.iter().find(|s| s.name == m.name) {
            if m.id.as_ref() != Some(&s.id) {
                m.id = Some(s.id.clone());
                changed = true;
            }
            kept.push(m);
            continue;
        }
        changed = true;
    }
    (kept, changed)
}

/// Stamp ids onto name-only rows from the current live list. Does not prune.
fn stamp_ids(members: &mut [Member], live: &[Session]) -> bool {
    let mut changed = false;
    for m in members {
        if m.id.is_none() {
            if let Some(s) = live.iter().find(|s| s.name == m.name) {
                m.id = Some(s.id.clone());
                changed = true;
            }
        }
    }
    changed
}

/// Write session ids onto every member that currently exists in tmux, so a
/// following rename can be matched by id instead of the old name.
pub fn stamp_group_ids(group: &str) {
    if !server_alive() {
        return;
    }
    let live = list_session_rows();
    let mut members = load_raw(group);
    if stamp_ids(&mut members, &live) {
        write_members(group, &members);
    }
}

/// Sessions registered to `group`, in original order. Prunes any that no
/// longer exist in tmux (rewriting the group file) as long as the server is
/// actually reachable, so a down server never wipes a group's membership.
pub fn group_members(group: &str) -> Vec<String> {
    let members = load_raw(group);
    if !server_alive() {
        return members.into_iter().map(|m| m.name).collect();
    }
    let live = list_session_rows();
    let (kept, changed) = resolve_members(members, &live);
    if changed {
        write_members(group, &kept);
    }
    kept.into_iter().map(|m| m.name).collect()
}

pub fn add_member(group: &str, session: &str) {
    let live = list_session_rows();
    let mut members = load_raw(group);
    let _ = stamp_ids(&mut members, &live);
    let sid = live.iter().find(|s| s.name == session).map(|s| s.id.clone());
    let exists = members.iter().any(|m| {
        m.name == session || (sid.is_some() && m.id.is_some() && m.id == sid)
    });
    if !exists {
        members.push(Member {
            id: sid,
            name: session.to_string(),
        });
    }
    write_members(group, &members);
}

pub fn remove_member(group: &str, session: &str) {
    let live = list_session_rows();
    let sid = live.iter().find(|s| s.name == session).map(|s| s.id.clone());
    let mut members = load_raw(group);
    let before = members.len();
    members.retain(|m| {
        m.name != session && !(sid.is_some() && m.id.is_some() && m.id == sid)
    });
    if members.len() != before {
        write_members(group, &members);
    }
}

/// Updates the stored name after a tmux rename. Reads the raw file (no prune)
/// and matches by session id when known, otherwise by the old name.
pub fn rename_member(group: &str, old: &str, new_name: &str) {
    let live = list_session_rows();
    let new_id = live.iter().find(|s| s.name == new_name).map(|s| s.id.clone());
    let mut members = load_raw(group);
    let mut changed = false;
    for m in &mut members {
        let by_id = matches!((&m.id, &new_id), (Some(id), Some(nid)) if id == nid);
        if by_id || m.name == old {
            m.name = new_name.to_string();
            if m.id.is_none() {
                m.id = new_id.clone();
            }
            changed = true;
        }
    }
    if changed {
        write_members(group, &members);
    }
}

/// Which group a session belongs to, if any.
pub fn group_of_session(session: &str) -> Option<String> {
    let sid = list_session_rows()
        .into_iter()
        .find(|s| s.name == session)
        .map(|s| s.id);
    let entries = std::fs::read_dir(groups_dir()).ok()?;
    for entry in entries.flatten() {
        let group = entry.file_name().to_string_lossy().to_string();
        if load_raw(&group).iter().any(|m| {
            m.name == session || (sid.is_some() && m.id.is_some() && m.id == sid)
        }) {
            return Some(group);
        }
    }
    None
}

/// All known groups, sorted by name.
pub fn list_groups() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(groups_dir())
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Creates a session and registers it into `group`. Reuses the group's own
/// name for the session when that name is free, so a brand new group's
/// first session reads naturally.
fn spawn_member(group: &str) -> String {
    let name = if !tmux_ok(&["has-session", "-t", &format!("={group}")]) {
        group.to_string()
    } else {
        unique_name()
    };
    tmux(&["new-session", "-d", "-s", &name]);
    add_member(group, &name);
    name
}

/// The tab-bar list for whichever group `current` belongs to (falls back to
/// just `current` alone if it isn't registered to any group yet).
pub fn members_for(current: &str) -> Vec<String> {
    let group = group_of_session(current).unwrap_or_else(|| DEFAULT_GROUP.to_string());
    let members = group_members(&group);
    if members.is_empty() {
        vec![current.to_string()]
    } else {
        members
    }
}

/// Ensures `group` has at least one live session, creating one if needed.
/// Returns the group's members (pruned, guaranteed non-empty).
pub fn ensure_group(group: &str) -> Vec<String> {
    let members = group_members(group);
    if !members.is_empty() {
        return members;
    }
    // Adopt a pre-existing, unregistered session named after the group
    // (e.g. the one created when bootstrapping a brand new server).
    if tmux_ok(&["has-session", "-t", &format!("={group}")]) && group_of_session(group).is_none() {
        add_member(group, group);
        return vec![group.to_string()];
    }
    vec![spawn_member(group)]
}

/// Closes one sub-session within its group. If it was the group's last
/// session, spawns a replacement first so the client never gets bounced
/// into an unrelated group. Returns the session the client ends up on.
pub fn close_session(session: &str, client: Option<&str>) -> Option<String> {
    let session = session.trim();
    if session.is_empty() {
        return None;
    }
    let group = group_of_session(session).unwrap_or_else(|| DEFAULT_GROUP.to_string());
    let members = group_members(&group);
    if !members.iter().any(|m| m == session) {
        return None;
    }
    let target = if members.len() > 1 {
        let idx = members.iter().position(|m| m == session).unwrap();
        members[(idx + members.len() - 1) % members.len()].clone()
    } else {
        spawn_member(&group)
    };
    switch_to(&target, client);
    tmux(&["kill-session", "-t", &format!("={session}")]);
    let _ = std::fs::remove_file(status_path(session));
    remove_member(&group, session);
    Some(target)
}

/// Kills every session in `group` and forgets the group entirely.
pub fn close_group(group: &str) -> usize {
    let members = group_members(group);
    for name in &members {
        tmux(&["kill-session", "-t", &format!("={name}")]);
        let _ = std::fs::remove_file(status_path(name));
    }
    let _ = std::fs::remove_file(group_path(group));
    members.len()
}

#[cfg(test)]
mod tests {
    use super::{parse_member, resolve_members, stamp_ids, Member};
    use crate::tmux::Session;

    fn live(pairs: &[(&str, &str)]) -> Vec<Session> {
        pairs
            .iter()
            .enumerate()
            .map(|(i, (id, name))| Session {
                id: (*id).into(),
                name: (*name).into(),
                created: i as i64,
            })
            .collect()
    }

    #[test]
    fn parse_id_and_legacy_name() {
        assert_eq!(
            parse_member("$3\top1"),
            Some(Member {
                id: Some("$3".into()),
                name: "op1".into()
            })
        );
        assert_eq!(
            parse_member("op1"),
            Some(Member {
                id: None,
                name: "op1".into()
            })
        );
    }

    #[test]
    fn rename_keeps_member_by_id() {
        let members = vec![Member {
            id: Some("$5".into()),
            name: "old".into(),
        }];
        let (kept, changed) = resolve_members(members, &live(&[("$5", "new")]));
        assert!(changed);
        assert_eq!(
            kept,
            vec![Member {
                id: Some("$5".into()),
                name: "new".into()
            }]
        );
    }

    #[test]
    fn prune_does_not_drop_renamed_id() {
        let members = vec![
            Member {
                id: Some("$1".into()),
                name: "keep".into(),
            },
            Member {
                id: Some("$9".into()),
                name: "gone".into(),
            },
        ];
        let (kept, changed) = resolve_members(members, &live(&[("$1", "keep")]));
        assert!(changed);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].name, "keep");
    }

    #[test]
    fn stamp_then_resolve_survives_rename() {
        let mut members = vec![Member {
            id: None,
            name: "old".into(),
        }];
        let before = live(&[("$7", "old")]);
        assert!(stamp_ids(&mut members, &before));
        assert_eq!(members[0].id.as_deref(), Some("$7"));
        let (kept, _) = resolve_members(members, &live(&[("$7", "renamed")]));
        assert_eq!(kept[0].name, "renamed");
    }
}
