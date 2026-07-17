#!/bin/bash
# initialize/rebuild-debug.sh — sourced by initialize.sh
#
# Provides dump_rebuild_context(), an opt-in diagnostic dump that records
# everything that could plausibly signal "rebuild without cache" from VS
# Code / Dev Container CLI. Written to .devcontainer/logs/rebuild-context-
# <ts>.log. Cheap (~20 ms), helps reverse-engineer how VS Code / devcontainer
# CLI propagates the --no-cache flag — we don't have docs for it.
#
# Enable via DEBUG_REBUILD_CONTEXT=1 in .devcontainer/.env or as env prefix.
# Off by default so .devcontainer/logs/ stays clean between builds. Remove
# once detection is robust.
#
# Reads : DEVCONTAINER_DIR (set by initialize.sh), $$, PPID, env, ancestry.
# Writes: rebuild-context-<ts>.log + one echo to stdout (log path pointer).

dump_rebuild_context() {
	mkdir -p "$DEVCONTAINER_DIR/logs"
	local ts log
	ts=$(date +%Y%m%d-%H%M%S)
	log="$DEVCONTAINER_DIR/logs/rebuild-context-${ts}.log"
	{
		echo "=== rebuild-context $(date) ==="
		echo "self pid       : $$"
		echo "self args      : $(ps -o args= -p "$$" 2>/dev/null)"
		echo "self ppid      : $PPID"
		echo
		echo "=== full ancestry (ps walk up to PID 1, max 15 hops) ==="
		local pid=$$ depth=0
		while [ "$pid" -gt 1 ] && [ "$depth" -lt 15 ]; do
			local p_args p_ppid
			p_args=$(ps -o args= -p "$pid" 2>/dev/null || echo "<unavailable>")
			p_ppid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' \t' || echo "")
			printf 'depth=%-2d pid=%-7d ppid=%-7s\n' "$depth" "$pid" "${p_ppid:-?}"
			printf '  args : %s\n' "$p_args"
			pid="$p_ppid"
			[ -z "$pid" ] && break
			depth=$((depth + 1))
		done
		echo
		echo "=== env vars (VS Code / Dev Container / Docker / Compose / Buildkit / no-cache) ==="
		env | grep -iE '(VSCODE|REMOTE_|DEVCONTAINER|DOCKER|COMPOSE|BUILDKIT|NO_?CACHE|REBUILD|CI)=' \
			| sort \
			|| echo "(no matches)"
		echo
		echo "=== all env keys (names only, sorted) ==="
		env | cut -d= -f1 | sort | column -c "${COLUMNS:-100}" 2>/dev/null \
			|| env | cut -d= -f1 | sort | tr '\n' ' ' | fold -s -w "${COLUMNS:-100}"
		echo
		echo "=== signal files in .devcontainer/ ==="
		find "$DEVCONTAINER_DIR" -maxdepth 2 -name '.*' -type f 2>/dev/null | sort
		echo
		echo "=== shell context ==="
		echo "BASH_SOURCE     : ${BASH_SOURCE[*]:-<unset>}"
		echo "0               : $0"
		echo "BASH_VERSION    : ${BASH_VERSION:-<not bash>}"
		echo "SHLVL           : ${SHLVL:-?}"
		echo "TERM            : ${TERM:-?}"
		echo "TTY -t 1 (stdout): $([ -t 1 ] && echo yes || echo no)"
		echo "TTY -t 0 (stdin) : $([ -t 0 ] && echo yes || echo no)"
		echo "=== end ==="
	} > "$log" 2>&1
	echo "ℹ Rebuild context dumped to: ${log#${DEVCONTAINER_DIR}/}"
}
