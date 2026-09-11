#!/bin/sh
# tabmux status-dot helper for Claude Code hooks.
# Usage: tabmux-status.sh <busy|attention|idle|unset>
# Override the binary with TABMUX_BIN if tabmux is not on PATH.
set -eu
state="${1:-}"
if [ -z "$state" ]; then
	echo "usage: tabmux-status.sh <busy|attention|idle|unset>" >&2
	exit 2
fi
bin="${TABMUX_BIN:-$HOME/bin/tabmux}"
if [ ! -x "$bin" ]; then
	bin="tabmux"
fi
exec "$bin" status "$state"
