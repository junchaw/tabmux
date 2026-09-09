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
tabmux              # attach (creates the server if needed)
tabmux ls           # list sessions
tabmux new [name]   # create and switch
tabmux close [name] # kill a session (keeps the last one)
tabmux status <busy|attention|idle> [session]
                    # set a session's status dot; session defaults to the calling pane's
```

## Status dot

A session's tab can show a colored dot to signal what a long-running process
(an agent, a build, anything) is doing:

```sh
tabmux status busy       # yellow dot, e.g. before a long task starts
tabmux status attention  # red dot, e.g. needs input or failed
tabmux status idle       # clears the dot
```

Run without a session argument, this reads the session from the calling pane
(`$TMUX_PANE`), so a script running inside a tab can just call `tabmux status busy`
on its own behalf. Status is stored per-session in `~/.config/tabmux/status/`.

## Inside the app

| Key | Action |
|---|---|
| `Ctrl-b` | full-screen command menu |
| left-click a tab | switch session |
| right-click a tab | close session |
| `n` | new session |
| `x` | close current session |
| `1`–`9` | jump to nth tab |
| `p` | previous session |
| `d` | detach |
| `q` / `Esc` | close the menu |

The bottom row is a right-aligned message ticker (`Try Ctrl+B` stays pinned on the left).
