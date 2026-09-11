# Claude Code status-dot hook

This makes a tabmux tab show a status dot while [Claude Code](https://code.claude.com/docs/en/overview) is running in that pane. It uses Claude Code **hooks** in `~/.claude/settings.json`, not a change to Claude Code itself.

| When | Dot |
|---|---|
| session start / turn finished | green (`idle`) |
| user submitted a prompt | red (`busy`) |
| Claude sent a notification (permission prompt, idle wait) | yellow (`attention`) |
| session ended | hidden (`unset`) |

## Install

From a clone of this repo:

```sh
mkdir -p ~/.claude
cp contrib/claude/tabmux-status.sh ~/.claude/tabmux-status.sh
chmod +x ~/.claude/tabmux-status.sh
```

Then merge the hook block from `contrib/claude/hooks.json` into `~/.claude/settings.json` under a top-level `"hooks"` key. If you already have `"hooks"`, add these events next to your existing ones instead of replacing the file.

Minimal `~/.claude/settings.json` if you have none:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "$HOME/.claude/tabmux-status.sh idle", "timeout": 5 }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          { "type": "command", "command": "$HOME/.claude/tabmux-status.sh busy", "timeout": 5 }
        ]
      }
    ],
    "Notification": [
      {
        "hooks": [
          { "type": "command", "command": "$HOME/.claude/tabmux-status.sh attention", "timeout": 5 }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "$HOME/.claude/tabmux-status.sh idle", "timeout": 5 }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          { "type": "command", "command": "$HOME/.claude/tabmux-status.sh unset", "timeout": 5 }
        ]
      }
    ]
  }
}
```

Restart Claude Code so it reloads settings.

If `~/bin/tabmux` is not the binary you want, set `TABMUX_BIN` in the environment Claude Code inherits.

You must be **inside a tabmux pane**. The hook runs `tabmux status <state>` with no session name; tabmux uses `$TMUX_PANE`.

## What it calls

These are Claude Code lifecycle hooks, not the bash tool, so they are not permission-gated as model-run commands.

```sh
tabmux status busy
tabmux status attention
tabmux status idle
tabmux status unset
```
