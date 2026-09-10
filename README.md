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
tabmux status <name> [session]
                    # set this tab's status dot (busy/attention/idle/unset or any name)
tabmux save         # write session names and cwd (also on detach) so a restart can recreate tabs
```

Each group is its own set of tabs. `tabmux attach new` opens (or creates) the
`new` group without touching `default`. New tabs in a named group are
`{group}-s1`, `{group}-s2`, …; `default` still uses `s1`, `s2`.

## Status dot

A tab can show a colored dot. Built-in names:

```sh
tabmux status busy       # red
tabmux status attention  # yellow
tabmux status idle       # green
tabmux status unset      # hide the dot
tabmux status review     # any other name: cyan unless mapped
```

Without a session argument this uses `$TMUX_PANE`. Extra names and colors go in
`~/.config/tabmux/status-colors` (`name colour196` per line).

To drive the dot from [pi](https://github.com/badlogic/pi-mono) automatically,
install the extension in `docs/pi.md` (template: `contrib/pi/tabmux.ts`).

## Restoring sessions after a restart

tabmux keeps each session's name and working directory in
`~/.config/tabmux/sessions`. `tabmux save` writes that file; detach does too.
If the tmux server is gone (`kill-server`, reboot, crash), the next attach
recreates each session at its saved path. Layout only — not what was running
in the pane.

## Inside the app

Press `Ctrl-b` for the command menu.
