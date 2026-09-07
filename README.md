# tabmux

Isolated tmux with a bottom session tab bar and a message ticker.

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
```

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
