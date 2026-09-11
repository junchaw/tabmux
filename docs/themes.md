# Custom themes

A theme colors the session tabs, the Ctrl+B / events bar, and status dots.
Built-in names: `nord`, `catppuccin-mocha`, `tokyonight`, `gruvbox-dark`,
`dracula`, `solarized-dark`, `rose-pine`.

## Install a file

JSON or YAML. Put it in `~/.config/tabmux/themes/` (any `*.json` / `*.yaml` /
`*.yml`) and it shows up in the theme list, or pick **from file…** and type a
path.

```sh
mkdir -p ~/.config/tabmux/themes
cp contrib/themes/example.json ~/.config/tabmux/themes/my.json
```

Then Ctrl-b → `c` → theme, or `tabmux config reset` and choose it on first attach.

`theme=` in `~/.config/tabmux/config` (global) or
`~/.config/tabmux/config.groups/<group>` can be a builtin id, a stem in the
themes dir (`my`), or an absolute path.

## File format

All keys optional. Missing keys fall back to `nord`.

```json
{
  "name": "my-theme",
  "active_bg": "#89b4fa",
  "active_fg": "#1e1e2e",
  "inactive_bg": "#313244",
  "inactive_fg": "#cdd6f4",
  "hint_bg": "#f9e2af",
  "hint_fg": "#1e1e2e",
  "msg_bg": "#1e1e2e",
  "msg_fg": "#cdd6f4",
  "status_busy": "#f38ba8",
  "status_attention": "#f9e2af",
  "status_idle": "#a6e3a1",
  "status_default": "#89b4fa"
}
```

Same keys in YAML (`contrib/themes/example.yaml`).

| Key | Where it shows |
|---|---|
| `active_bg` / `active_fg` | Current session tab |
| `inactive_bg` / `inactive_fg` | Other tabs |
| `hint_bg` / `hint_fg` | `Try Ctrl+B` chip |
| `msg_bg` / `msg_fg` | Events ticker |
| `status_busy` | Red-style dot (`tabmux status busy`) |
| `status_attention` | Yellow-style dot |
| `status_idle` | Green-style dot |
| `status_default` | Any other status name |

Colors are tmux values: `#rrggbb` or `colour0`–`colour255`.

Per-status overrides still live in `~/.config/tabmux/status-colors`
(`name colour196` per line) and win over the theme.
