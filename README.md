# tabmux

Isolated tmux with a bottom session tab bar and a message ticker.

```
+------------------------------------------------------------------------------+
| $ ssh gpu-box                                                                |
| connected.                                                                   |
|                                                                              |
+-------------------+-------------------+-------------------+------------------+
|     main (1)      |  tabmux dev (2)   |  claude code (3)  |   pi agent (4)   |
+-------------+----------------------------------------------------------------+
| Try Ctrl+B  |                     2m created pi agent | 4s switched to main  |
+-------------+----------------------------------------------------------------+
```

Sessions live in a private tmux server (`tmux -L tabmux`), so your default tmux
sessions are left alone.

## Build

```sh
cargo build --release
cp target/release/tabmux ~/bin/tabmux
```

Requires Rust 1.70+ and tmux 3.2+.

## Usage

```
tabmux              # attach to the default group
tabmux attach [xx]  # attach to group xx (no name = default)
tabmux ls           # list groups
tabmux close <xx>   # kill group xx and all its sessions
tabmux new [name]   # create a tab in the current group
tabmux save         # snapshot session names/paths (also done automatically)
```

Each group is its own set of tabs. `tabmux attach new` opens (or creates) the
`new` group without touching `default`. New tabs in a named group are
`{group}-s1`, `{group}-s2`, …; `default` still uses `s1`, `s2`.

## Restoring sessions after a restart

tabmux keeps a snapshot of each session's name and working directory in
`~/.config/tabmux/sessions`, updated on new/close/rename and on detach. If
the tmux server itself goes away (`kill-server`, reboot, crash), the next
`tabmux` attach recreates each session at its saved path. This restores
layout only, not what was running in the pane.

## Inside the app

| Key | Action |
|---|---|
| `Ctrl-b` | full-screen command menu |
| left-click a tab | switch session |
| right-click a tab | close session |
| `n` | new session |
| `r` | rename current session |
| `m` | reorder sessions (`j`/`k` or arrows, Enter to save) |
| `x` | close current session |
| `1`–`9` | jump to nth tab |
| `p` | previous session |
| `d` | detach |
| `q` / `Esc` | close the menu |

The bottom row is a right-aligned message ticker (`Try Ctrl+B` stays pinned on the left).
