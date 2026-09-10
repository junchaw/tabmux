# Pi status-dot hook

This makes a tabmux tab show a status dot while [pi](https://github.com/badlogic/pi-mono) is running in that pane. It is a **pi extension**, not a change to pi itself.

| When | Dot |
|---|---|
| pi session open, idle | green (`idle`) |
| agent running | red (`busy`) |
| pi waiting on a UI prompt | yellow (`attention`) |
| pi session ended | hidden (`unset`) |

## Install

Copy the template into pi's global extensions directory, then restart pi (or `/reload`):

```sh
mkdir -p ~/.pi/agent/extensions
cp contrib/pi/tabmux.ts ~/.pi/agent/extensions/tabmux.ts
```

From a clone of this repo, that path is relative to the repo root. The file must be named `*.ts` under `~/.pi/agent/extensions/` so pi auto-discovers it.

If `tabmux` is not on `PATH` when pi starts, set:

```sh
export TABMUX_BIN=$HOME/bin/tabmux
```

You must be **inside a tabmux pane**. The extension runs `tabmux status <state>` with no session name; tabmux uses `$TMUX_PANE`.

## What it calls

The extension never goes through pi's bash tool (so it is not permission-gated). It `pi.exec`s:

```sh
tabmux status busy
tabmux status attention
tabmux status idle
tabmux status unset
```

Same commands work from any other agent or script.

## Other agents

Point the same CLI at your tool's lifecycle:

- start of a long run → `tabmux status busy`
- needs input → `tabmux status attention`
- finished but still attached → `tabmux status idle`
- process exit → `tabmux status unset`
