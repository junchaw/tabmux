/**
 * tabmux status-dot hook for pi
 *
 * Copy to ~/.pi/agent/extensions/tabmux.ts (see docs/pi.md).
 * Calls `tabmux status` so this pane's tab shows:
 *   busy       red    — agent is running
 *   attention  yellow — pi is waiting on a UI prompt
 *   idle       green  — pi is open but settled
 *   unset      hidden — pi session ended
 *
 * Override the binary with TABMUX_BIN if tabmux is not on PATH.
 */

import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

function setStatus(pi: ExtensionAPI, state: string) {
	const bin = process.env.TABMUX_BIN || "tabmux";
	void pi.exec(bin, ["status", state]).catch(() => {});
}

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async () => {
		setStatus(pi, "idle");
	});

	pi.on("agent_start", async () => {
		setStatus(pi, "busy");
	});

	pi.on("ui_prompt_start", async () => {
		setStatus(pi, "attention");
	});

	pi.on("ui_prompt_end", async () => {
		setStatus(pi, "busy");
	});

	pi.on("agent_settled", async () => {
		setStatus(pi, "idle");
	});

	pi.on("session_shutdown", async () => {
		setStatus(pi, "unset");
	});
}
