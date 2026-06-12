#!/bin/bash
# DevContainer initialization — runs on the HOST before container build
# Handles: Docker volumes, auth setup, Claude mode selection, firewall mode
# set -e : exit on first non-zero
# set -E : propagate the ERR trap into shell functions (without -E, a crash
#          inside detect_no_cache_request / build_base_if_missing / etc. would
#          die silently without printing the failing line).
set -eE

DEVCONTAINER_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$DEVCONTAINER_DIR/.." && pwd)"
ENV_FILE="$DEVCONTAINER_DIR/.env"

# === Host-OS detection ===
# HOST_KIND : mac | linux | wsl | gitbash | cygwin | unknown
# mac + linux are the historical targets. wsl + gitbash are Windows shims
# (script + daemon run under either WSL bash or MSYS2 Git Bash). cygwin +
# unknown are unsupported — the script aborts with one actionable line
# instead of failing later with cryptic command-not-found errors.
case "$(uname -s)" in
	Darwin)        HOST_KIND=mac ;;
	Linux)
		if [ -n "${WSL_DISTRO_NAME:-}" ] \
		   || [ -n "${WSL_INTEROP:-}" ] \
		   || ( [ -r /proc/version ] && grep -qi 'microsoft' /proc/version ); then
			HOST_KIND=wsl
		else
			HOST_KIND=linux
		fi
		;;
	MINGW*|MSYS*)  HOST_KIND=gitbash ;;
	CYGWIN*)       HOST_KIND=cygwin ;;
	*)             HOST_KIND=unknown ;;
esac

if [ "$HOST_KIND" = "cygwin" ] || [ "$HOST_KIND" = "unknown" ]; then
	echo "✗ initialize.sh does not support host kind: $HOST_KIND ($(uname -s))" >&2
	echo "  Supported : Mac, native Linux, Windows-with-WSL, Windows-with-Git-Bash." >&2
	exit 1
fi
export HOST_KIND

# === Path translation : POSIX → host-OS native ===
# Docker Desktop on Windows labels containers with Windows-format paths
# (C:\foo\bar). Bash holds them POSIX-style (/mnt/c/foo/bar in WSL,
# /c/foo/bar in Git Bash). Translate on demand so `docker ps --filter
# label=…` matches what the daemon wrote. Identity on Mac + native Linux.
to_host_path() {
	local p="$1"
	case "$HOST_KIND" in
		wsl)     wslpath -w "$p" ;;
		gitbash) cygpath -w "$p" ;;
		*)       printf '%s' "$p" ;;
	esac
}

# === Lifecycle logging ===
# Always-on : human-readable output in <hook>-<ts>.log (stdout+stderr via tee).
# DEBUG=1 only : xtrace in <hook>-<ts>.trace (every shell command + file:line).
# Set DEBUG=1 in .devcontainer/.env (or env-prefix the invocation) to enable
# the trace channel — the trace files are gitignored under .devcontainer/logs/.
# Same gating pattern in on-create.sh, post-create.sh, post-start.sh.
#
# When DEBUG=1 : BASH_XTRACEFD writes xtrace to a separate fd so the terminal
# stays clean. Falls back to inline xtrace (mixed into .log) on bash < 4.1
# (macOS default 3.2 has no BASH_XTRACEFD).
mkdir -p "$DEVCONTAINER_DIR/logs"
TS=$(date +%Y%m%d-%H%M%S)
INIT_LOG="$DEVCONTAINER_DIR/logs/initialize-${TS}.log"
INIT_TRACE="$DEVCONTAINER_DIR/logs/initialize-${TS}.trace"
# Save fd 3 = original terminal stdout BEFORE the tee redirect so
# run_with_progress can write rolling-window cursor escapes directly to the
# terminal (fd 3) without polluting INIT_LOG with ANSI codes. Also capture
# the original TTY-ness for the function's branching — `[ -t 1 ]` post-tee
# is always false (fd 1 = pipe to tee).
if [ -t 1 ]; then export ORIG_STDOUT_TTY=1; else export ORIG_STDOUT_TTY=0; fi
exec 3>&1
exec > >(tee -a "$INIT_LOG") 2>&1
if [ "${DEBUG:-0}" = "1" ]; then
	PS4='+ ${BASH_SOURCE##*/}:${LINENO}: '
	if (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 1) )); then
		exec 19>>"$INIT_TRACE"
		export BASH_XTRACEFD=19
		TRACE_LOC="${INIT_TRACE#${DEVCONTAINER_DIR}/}"
	else
		# bash 3.x : no BASH_XTRACEFD → xtrace flows through stderr → tee → .log.
		rm -f "$INIT_TRACE"
		TRACE_LOC="mixed into .log (bash ${BASH_VERSION%%(*} — grep '^+ ' to filter)"
	fi
	set -x
else
	rm -f "$INIT_TRACE"
	TRACE_LOC="(disabled — set DEBUG=1 in .env to enable xtrace)"
fi
# ERR trap stays on regardless of DEBUG — when set -e exits non-zero it
# prints failing line + command + exit code to the human log.
trap 'rc=$?; [ $rc -ne 0 ] && echo "✗ FAIL at ${BASH_SOURCE##*/}:${LINENO} (exit $rc): $BASH_COMMAND" >&2' ERR
echo "=== initialize.sh $(date) ==="
echo "  log:   ${INIT_LOG#${DEVCONTAINER_DIR}/}"
echo "  trace: $TRACE_LOC"

# Source .devcontainer/.env to get DC_PROJECT, CLAUDE_CREDS_VOLUME, etc.
if [ -f "$ENV_FILE" ]; then
	set -a
	source "$ENV_FILE"
	set +a
fi

CREDS_VOLUME="${CLAUDE_CREDS_VOLUME:-claude-creds-${DC_PROJECT:-devcontainer-tools}}"

echo "  DC_PROJECT:   ${DC_PROJECT:-devcontainer-tools}"
echo "  claude-creds: $CREDS_VOLUME"

# Ensure firewall/ baked files exist before Dockerfile COPY (recursive).
# domains.local.txt is gitignored ; default-mode + direct-tcp-allow.txt are
# committed but a fresh project clone may need them seeded. policy.local.d/
# is committed with .keep, but mkdir as a safety net for older clones.
[ -f "$DEVCONTAINER_DIR/firewall/domains.local.txt" ] || \
  touch "$DEVCONTAINER_DIR/firewall/domains.local.txt"
[ -s "$DEVCONTAINER_DIR/firewall/default-mode" ] || \
  echo "strict" > "$DEVCONTAINER_DIR/firewall/default-mode"
[ -f "$DEVCONTAINER_DIR/firewall/direct-tcp-allow.txt" ] || \
  : > "$DEVCONTAINER_DIR/firewall/direct-tcp-allow.txt"
mkdir -p "$DEVCONTAINER_DIR/firewall/policy.local.d"

# Migrate legacy .configured-firewall-mode → firewall/default-mode (one-shot,
# idempotent). After this migration, the flag file is the baked source of truth.
LEGACY_FW_FLAG="$DEVCONTAINER_DIR/.configured-firewall-mode"
if [ -f "$LEGACY_FW_FLAG" ] && [ ! -s "$DEVCONTAINER_DIR/firewall/default-mode" ]; then
  cp "$LEGACY_FW_FLAG" "$DEVCONTAINER_DIR/firewall/default-mode"
  echo "→ migrated firewall mode : .configured-firewall-mode → firewall/default-mode"
fi

# Ensure claude-bridge sidecar config exists BEFORE compose runs — Docker
# would otherwise create an empty directory at the bind mount path
# (./claude-bridge/config.json:/app/config.json:ro) and the sidecar would
# crash trying to read /app/config.json. Auto-copy from the committed
# template ; manual user edits to config.json are preserved (the cp only
# fires when the file is absent).
CB_CONFIG_DIR="$DEVCONTAINER_DIR/claude-bridge"
if [ -f "$CB_CONFIG_DIR/config.example.json" ] && [ ! -f "$CB_CONFIG_DIR/config.json" ]; then
	cp "$CB_CONFIG_DIR/config.example.json" "$CB_CONFIG_DIR/config.json"
	echo "✓ Bootstrapped claude-bridge/config.json from config.example.json"
fi

# Create volume (always needed)
docker volume create "$CREDS_VOLUME" > /dev/null 2>&1 || true

# Flag files — delete to re-prompt on next rebuild.
# FW_FLAG moved into firewall/ + renamed default-mode since session 1 (bake-only).
AUTH_FLAG="$DEVCONTAINER_DIR/.configured-auth"
MODE_FLAG="$DEVCONTAINER_DIR/.configured-claude-mode"
FW_FLAG="$DEVCONTAINER_DIR/firewall/default-mode"

# -----------------------------------------------
# Helpers — .env file management
# -----------------------------------------------

# Idempotent KEY=VALUE write into .env. Always ensures trailing newline first
# (avoids concatenation bug if .env doesn't end with \n). Uses a temp file
# instead of `sed -i` for portability between GNU sed (Linux) and BSD sed
# (macOS — initialize.sh runs on the host).
set_env_var() {
	local key="$1" value="$2" file="${3:-$ENV_FILE}"
	touch "$file"
	if [ -s "$file" ] && [ "$(tail -c 1 "$file" | od -An -c | tr -d ' ')" != "\n" ]; then
		echo "" >> "$file"
	fi
	if grep -q "^${key}=" "$file"; then
		local tmp; tmp=$(mktemp)
		awk -v k="$key" -v v="$value" '
			$0 ~ "^"k"=" { print k"="v; next }
			{ print }
		' "$file" > "$tmp" && mv "$tmp" "$file"
	else
		echo "${key}=${value}" >> "$file"
	fi
}

unset_env_var() {
	local key="$1" file="${2:-$ENV_FILE}"
	[ -f "$file" ] || return 0
	local tmp; tmp=$(mktemp)
	grep -v "^${key}=" "$file" > "$tmp" || true
	mv "$tmp" "$file"
}

# Sync .env proxy/CA vars to the firewall flag (idempotent — safe to call
# at any time, e.g. after a manual flag edit).
#
# Modes (canonical → behaviour) :
#   strict (alias: paranoid)  — proxy/CA vars set
#   basic  (alias: okeish)    — vars cleared
#   off                       — vars cleared
sync_proxy_env() {
	local mode="${1:-strict}"
	case "$mode" in
		strict|paranoid)
			set_env_var "HTTPS_PROXY" "http://127.0.0.1:8080"
			set_env_var "HTTP_PROXY" "http://127.0.0.1:8080"
			set_env_var "NO_PROXY" "localhost,127.0.0.0/8,host.docker.internal,.local"
			set_env_var "NODE_EXTRA_CA_CERTS" "/var/lib/mitmproxy/mitmproxy-ca-cert.pem"
			;;
		*)
			unset_env_var "HTTPS_PROXY"
			unset_env_var "HTTP_PROXY"
			unset_env_var "NO_PROXY"
			unset_env_var "NODE_EXTRA_CA_CERTS"
			;;
	esac
}

# Rolling N-line progress window. Fixed banner above, last $window_size lines
# of the command's stdout/stderr below in dim gray, redrawn in place. Falls
# back to plain `tee` when no original TTY (CI, VS Code output channels).
# The full output is always teed to $log regardless of TTY.
#
# After initialize.sh's tee redirect, `[ -t 1 ]` always reports false because
# fd 1 = pipe to tee. The function therefore checks ORIG_STDOUT_TTY (captured
# pre-redirect) and writes ALL cursor-escape redraws to fd 3 (saved original
# stdout = terminal) so the .log file stays clean (no ANSI noise) while the
# user still sees the rolling window in the terminal.
#
# Usage: run_with_progress <log> <window_size> <title> -- <cmd> [args...]
run_with_progress() {
	local log="$1" window_size="$2" title="$3"
	shift 3
	# Skip the literal "--" separator if present (purely for readability at call sites).
	[ "${1:-}" = "--" ] && shift

	if [ "${ORIG_STDOUT_TTY:-0}" != "1" ]; then
		# No interactive TTY → plain tee, no fancy redraws (would render as garbage)
		printf '▸ %s\n' "$title"
		{ "$@"; } 2>&1 | tee -a "$log"
		return ${PIPESTATUS[0]}
	fi

	local cols=${COLUMNS:-$(tput cols 2>/dev/null || echo 100)}
	local maxw=$((cols - 4))
	[ "$maxw" -lt 40 ] && maxw=40

	# Banner stays on stdout (= teed to INIT_LOG + terminal). The rolling
	# redraws below all go to fd 3 only (terminal, no .log pollution).
	printf '\033[1;36m▸ %s\033[0m\n' "$title"
	local _i
	for ((_i = 0; _i < window_size; _i++)); do printf '\n' >&3; done

	set -o pipefail
	{
		"$@" 2>&1 | tee -a "$log" | {
			local -a buffer=()
			local line _b _count
			while IFS= read -r line; do
				line="${line%$'\r'}"
				# Push line, drop oldest when full
				if [ ${#buffer[@]} -ge "$window_size" ]; then
					buffer=("${buffer[@]:1}")
				fi
				buffer+=("$line")
				# Move cursor up to start of the window (terminal only)
				printf '\033[%dF' "$window_size" >&3
				# Redraw buffer (dim gray, truncated to maxw to avoid wrap)
				_count=${#buffer[@]}
				for _b in "${buffer[@]}"; do
					printf '\033[2K\033[2;37m  %.*s\033[0m\n' "$maxw" "$_b" >&3
				done
				# Pad remaining slots with cleared empty lines
				for ((_i = _count; _i < window_size; _i++)); do
					printf '\033[2K\n' >&3
				done
			done
		}
	}
	local rc=${PIPESTATUS[0]}
	set +o pipefail

	# Clear the window when done (success or failure), leaving banner visible.
	printf '\033[%dF' "$window_size" >&3
	for ((_i = 0; _i < window_size; _i++)); do printf '\033[2K\n' >&3; done
	printf '\033[%dF' "$window_size" >&3

	return "$rc"
}

# DIAG dump : record everything that could plausibly signal "rebuild without
# cache". Written unconditionally to .devcontainer/logs/rebuild-context-<ts>.log
# at every initialize.sh run. Cheap (~20 ms), helps reverse-engineer how VS
# Code / devcontainer CLI propagates the --no-cache flag — we don't have docs
# for it. Remove once detection is robust.
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

# Detect "Rebuild Container" vs "Reopen in Container" vs first-time.
#
# Surprise : VS Code does NOT pass a distinguishing flag to devcontainer CLI.
# Both "Rebuild Container" and "Reopen in Container" invoke exactly
# `devContainersSpecCLI.js up <usual flags>`. The actual rebuild semantic is
# that VS Code stops+removes the existing container BEFORE calling `up`, so
# the CLI ends up recreating the container — but transparently to our script.
# (Verified live in initialize-20260520-191649.log : zero rebuild flag in
# the parent process args after a confirmed user-clicked "Rebuild Container".)
#
# Therefore the reliable signal is the container itself :
#   - existing matching container present → Reopen (skip)
#   - no matching container               → first-time OR Rebuild (build)
# The container is matched by Docker labels VS Code sets unconditionally :
#   devcontainer.local_folder=<absolute workspace path>
#   devcontainer.config_file=<absolute devcontainer.json path>
#
# Separately, "Rebuild Without Cache" DOES propagate --no-cache /
# --build-no-cache through the process tree (confirmed) so we still walk the
# ancestry for that to set BUILD_BASE_NO_CACHE.
#
# Exported signals :
#   BUILD_BASE_REQUESTED=1   → rebuild base (cache OK unless …)
#   BUILD_BASE_NO_CACHE=1    → rebuild base with --no-cache
#
# Portable across Linux + macOS (no /proc dependency, ps + docker everywhere).
#
# Windows shims (WSL / Git Bash) : Channel 1's ps ancestor walk only sees the
# shim's own subprocess tree — VS Code on the Windows host is unreachable and
# the walk silently terminates without finding --no-cache. Channel 2 (docker
# ps probe, path-translated via to_host_path so the label filter matches the
# Windows-format paths VS Code wrote) is the reliable signal on those hosts.
# Mac + native Linux see the full ancestry and both channels work.
detect_no_cache_request() {
	# Explicit env override wins on both signals.
	if [ "${BUILD_BASE_NO_CACHE:-0}" = "1" ]; then
		export BUILD_BASE_REQUESTED=1
		return 0
	fi

	# Channel 1 — ancestor walk for --no-cache (orchestrator filter to avoid
	# false positives from unrelated parents carrying the literal string).
	local pid=$$ depth=0 max_depth=8 args
	local orchestrator_re='(devcontainer|docker|compose|buildkit|code helper)'
	while [ "$pid" -gt 1 ] && [ "$depth" -lt "$max_depth" ]; do
		args=$(ps -o args= -p "$pid" 2>/dev/null) || break
		if printf '%s' "$args" | grep -qiE -- "$orchestrator_re" \
		   && printf '%s' "$args" | grep -qE -- '(--build-no-cache|--no-cache)([[:space:]]|$)'; then
			export BUILD_BASE_NO_CACHE=1
			export BUILD_BASE_REQUESTED=1
			echo "  ↳ Detected --no-cache request (depth $depth)"
			break
		fi
		pid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' \t')
		[ -z "$pid" ] && break
		depth=$((depth + 1))
	done

	# Channel 2 — container existence probe. `docker ps -a` returns one ID
	# per matching container ; empty output = no match. Filters AND together,
	# so we get the exact container for THIS workspace + config.
	# Skip the probe if BUILD_BASE_NO_CACHE already won (no need to refine).
	if [ "${BUILD_BASE_NO_CACHE:-0}" != "1" ] && command -v docker >/dev/null 2>&1; then
		local ctr_id="" p_local p_cfg
		# Docker labels are written by VS Code in host-OS-native format
		# (C:\… on Windows). On WSL / Git Bash the POSIX form held in bash
		# (/mnt/c/…, /c/…) never matches → the filter silently returns no
		# hits and the daemon over-rebuilds. to_host_path() is identity on
		# Mac + native Linux, so this is a no-op on the historical paths.
		p_local=$(to_host_path "$PROJECT_DIR")
		p_cfg=$(to_host_path "$DEVCONTAINER_DIR/devcontainer.json")
		# Match the live devcontainer in ANY state. A "Restart VS Code" or
		# "Reopen in Container" stops the container BEFORE initializeCommand
		# runs (state=exited, ~seconds old) — without -a the probe misses it
		# and falsely triggers a rebuild. The com.docker.compose.project
		# label filter excludes manually-run zombies (`docker run` with the
		# devcontainer labels but no compose orchestration) — couples this
		# branch to compose mode, which this devcontainer.json always uses.
		ctr_id=$(docker ps -a -q \
			--filter "label=devcontainer.local_folder=$p_local" \
			--filter "label=devcontainer.config_file=$p_cfg" \
			--filter "label=com.docker.compose.project" \
			2>/dev/null | head -1)
		if [ -z "$ctr_id" ]; then
			export BUILD_BASE_REQUESTED=1
			echo "  ↳ No matching devcontainer for this workspace — rebuild or first-time"
		else
			echo "  ↳ Devcontainer present ($ctr_id, any state) — reopen, no base rebuild"
		fi
	fi

	# Always return 0 — signal is via env vars, not exit code (set -e would
	# kill the caller otherwise).

	# Channel 2 diagnostic snapshot (paths / docker ps / ancestry / env / verdict) —
	# uncomment the next line to capture into logs/rebuild-detect-<ts>.log.
	# "$DEVCONTAINER_DIR/diag-rebuild-detect.sh" "$TS" || true

	return 0
}

# Build claude-devcontainer-base:${VERSION} when slim variant is selected.
# Two trigger conditions :
#   - Tag missing (first-time setup) → build with cache.
#   - VS Code "Rebuild Container" detected → cached rebuild (~10-30s with
#     warm cache; only the COPY layer of an edited script + downstream
#     layers are invalidated).
#   - VS Code "Rebuild Container Without Cache" detected → full --no-cache
#     rebuild (~3-4 min).
# "Reopen in Container" with no rebuild flag → skip (instant).
#
# Strips HTTP(S)_PROXY before `docker build` because mitmproxy listens on
# 127.0.0.1:8080 INSIDE the runtime container only — leaking the var into the
# build would make apt/curl/npm inside the image try to reach a dead port
# (cf. amendment 2026-05-20). All output is teed to
# .devcontainer/logs/build-base-<version>-<ts>.log for post-mortem.
build_base_if_missing() {
	local version="${CLAUDE_CODE_VERSION:-2.1.145}"
	local tag="claude-devcontainer-base:${version}-${DC_PROJECT:-devcontainer-tools}"
	set_env_var "CLAUDE_CODE_VERSION" "$version"

	detect_no_cache_request

	# Consume BUILD_BASE_NO_CACHE=1 from .env if that's where it came from, so
	# the NEXT rebuild is normal (cached). Avoids the user being stuck in
	# permanent no-cache mode after toggling the flag once.
	if [ "${BUILD_BASE_NO_CACHE:-0}" = "1" ] \
	   && [ -f "$ENV_FILE" ] \
	   && grep -qE '^BUILD_BASE_NO_CACHE=1[[:space:]]*$' "$ENV_FILE"; then
		set_env_var "BUILD_BASE_NO_CACHE" "0"
		echo "  ↳ Consumed BUILD_BASE_NO_CACHE=1 from .env (reset to 0 for next rebuild)"
	fi

	if docker image inspect "$tag" >/dev/null 2>&1; then
		if [ "${BUILD_BASE_REQUESTED:-0}" != "1" ]; then
			echo "✓ Base image $tag present (no rebuild signal — skipping)"
			return 0
		fi
		if [ "${BUILD_BASE_NO_CACHE:-0}" = "1" ]; then
			echo "→ $tag exists but --no-cache requested — full rebuild"
		else
			echo "→ $tag exists but Rebuild Container detected — cached rebuild"
		fi
	fi

	local ts log
	ts=$(date +%Y%m%d-%H%M%S)
	mkdir -p "$DEVCONTAINER_DIR/logs"
	log="$DEVCONTAINER_DIR/logs/build-base-${version}-${ts}.log"

	# Metadata header → log only (not stdout, to keep banner clean).
	{
		echo "=== build-base $(date) ==="
		echo "version    : $version"
		echo "tag        : $tag"
		echo "dockerfile : $DEVCONTAINER_DIR/Dockerfile.base"
		echo "context    : $DEVCONTAINER_DIR"
		echo "docker     : $(docker --version 2>&1)"
		echo "host arch  : $(uname -m)"
		echo "proxy env (will be stripped from docker build) :"
		env | grep -iE '^(http_proxy|https_proxy|no_proxy)=' | sed 's/^/    /' || echo "    (none set)"
		echo "---"
	} > "$log"

	local started rc
	started=$SECONDS

	# Optional cache buster. Set BUILD_BASE_NO_CACHE=1 to force a full rebuild
	# (useful to exercise the rolling progress display, or to validate a fresh
	# layer chain after a Dockerfile.base edit). Docker `--no-cache` invalidates
	# from FROM downward — slow (~3-4 min) but deterministic.
	local nocache_args=()
	if [ -n "${BUILD_BASE_NO_CACHE:-}" ]; then
		nocache_args=(--no-cache)
		echo "  (BUILD_BASE_NO_CACHE=1 → forcing full rebuild, no layer reuse)"
	fi

	run_with_progress "$log" 10 "Building Claude Devcontainer Base v${version}  (log: ${log#${DEVCONTAINER_DIR}/})" \
		env -u HTTPS_PROXY -u HTTP_PROXY -u NO_PROXY \
			-u https_proxy -u http_proxy -u no_proxy \
			docker build \
				--progress=plain \
				"${nocache_args[@]}" \
				-f "$DEVCONTAINER_DIR/Dockerfile.base" \
				--build-arg CLAUDE_CODE_VERSION="$version" \
				-t "$tag" \
				"$DEVCONTAINER_DIR"
	rc=$?

	local elapsed=$((SECONDS - started))
	if [ "$rc" -eq 0 ]; then
		printf '\033[1;32m✓ Built %s in %ds — log: %s\033[0m\n' "$tag" "$elapsed" "$log"
		return 0
	fi
	printf '\033[1;31m✗ Build failed (exit %d, %ds elapsed). Full log: %s\033[0m\n' "$rc" "$elapsed" "$log"
	echo "  Tail of last 20 log lines :"
	tail -n 20 "$log" | sed 's/^/    /'
	return 1
}

# Count active local overrides (returns "<hosts>:<policy-files>").
# Hosts = non-comment, non-empty lines in domains.local.txt.
# Policy files = *.yaml files in policy.local.d/ (a single yaml may carry many rules).
count_local_overrides() {
	local domains_file="$DEVCONTAINER_DIR/firewall/domains.local.txt"
	local policy_dir="$DEVCONTAINER_DIR/firewall/policy.local.d"
	local hosts=0 policy=0
	# grep -c prints "0" on no match but exits 1 — using `|| true` to allow that
	# without re-printing, otherwise hosts would become "0\n0" and break -gt below.
	if [ -f "$domains_file" ]; then
		hosts=$(grep -cE "^[[:space:]]*[^#[:space:]]" "$domains_file" 2>/dev/null || true)
	fi
	if [ -d "$policy_dir" ]; then
		policy=$(find "$policy_dir" -maxdepth 1 -name "*.yaml" -type f 2>/dev/null | wc -l | tr -d ' ')
	fi
	printf '%s:%s' "${hosts:-0}" "${policy:-0}"
}

# -----------------------------------------------
# Per-flag prompts
# -----------------------------------------------

prompt_auth() {
	echo ""
	echo "=== GitHub Auth ==="
	echo "  Standard: open a terminal after startup and run 'gh auth login'."
	echo "  (gh-secure mode dropped in Phase 3 A3 — Level 1 strict blocks"
	echo "  POST github.com/* outside /anthropics/* at the firewall layer.)"
	echo ""
	echo "standard" > "$AUTH_FLAG"
	[ -f "$MODE_FLAG" ] || echo "CLAUDE-dev.md" > "$MODE_FLAG"
	echo "✓ Standard auth configured."
}

prompt_claude_mode() {
	echo ""
	echo "=== Claude Mode ==="
	echo ""
	echo "  [1] Dev       — full coding assistant (default)"
	echo "  [2] Reviewer  — code review + PR management"
	echo ""
	read -p "Choose [1/2] (default: 1): " CLAUDE_MODE
	CLAUDE_MODE="${CLAUDE_MODE:-1}"
	if [ "$CLAUDE_MODE" = "2" ]; then
		echo "CLAUDE-reviewer.md" > "$MODE_FLAG"
	else
		echo "CLAUDE-dev.md" > "$MODE_FLAG"
	fi
	echo "✓ Claude mode: $(cat "$MODE_FLAG")"
}

write_firewall_default() {
	# Default since A4 : strict (DNS allowlist + mitmproxy force-proxy + addons).
	# No prompt — the choice is intentional (max-security baseline). To flip to a
	# looser mode, use `firewall-mode.sh basic` or `firewall-mode.sh off` then
	# rebuild. Legacy mode names `paranoid`/`okeish` remain accepted as aliases.
	echo "strict" > "$FW_FLAG"
	sync_proxy_env strict
	echo "✓ Firewall mode: strict (default — flip via firewall-mode.sh)"
}

# -----------------------------------------------
# Summary
# -----------------------------------------------

print_summary() {
	local auth_mode="gh token only"
	local claude_label="dev"
	if [ -f "$MODE_FLAG" ]; then
		claude_label=$(sed 's/CLAUDE-//;s/\.md//' "$MODE_FLAG")
	fi
	local fw_mode="strict"
	[ -f "$FW_FLAG" ] && fw_mode=$(cat "$FW_FLAG")

	local overrides hosts policy
	overrides=$(count_local_overrides)
	hosts="${overrides%%:*}"
	policy="${overrides##*:}"

	echo ""
	echo "──────────────────────────────────"
	echo "  GitHub:           $auth_mode"
	echo "  Claude:           $claude_label"
	echo "  Firewall mode:    $fw_mode"
	if [ "$hosts" -gt 0 ] || [ "$policy" -gt 0 ]; then
		echo "  Local overrides:  ⚠  $hosts host(s) + $policy policy.local.d file(s)"
	else
		echo "  Local overrides:  (none)"
	fi
	echo "──────────────────────────────────"
	echo "  Flip firewall mode (then rebuild):"
	echo "    .devcontainer/firewall-mode.sh strict   # default (max security)"
	echo "    .devcontainer/firewall-mode.sh basic    # DNS allowlist only"
	echo "    .devcontainer/firewall-mode.sh off      # kill-switch (no filter)"
	echo "  Reconfigure (each can be reset independently):"
	echo "    rm .devcontainer/.configured-auth            # reset GitHub auth"
	echo "    rm .devcontainer/.configured-claude-mode     # reset Claude mode"
	echo "    rm .devcontainer/firewall/default-mode       # reset firewall mode"
	echo "  Then rebuild the container."
	echo "──────────────────────────────────"
}

# -----------------------------------------------
# Main flow
# -----------------------------------------------

# Diagnostic dump : opt-in via DEBUG_REBUILD_CONTEXT=1 in .env or env. Used
# once to reverse-engineer VS Code's --build-no-cache propagation (see
# .devcontainer/logs/rebuild-context-20260520-104648.log). Keep available for
# future detection regressions ; off by default so .devcontainer/logs/ stays
# clean between builds.
[ "${DEBUG_REBUILD_CONTEXT:-0}" = "1" ] && dump_rebuild_context

# Build claude-devcontainer-base if missing OR if VS Code triggered Rebuild
# Container. Runs before compose so the slim Dockerfile's FROM resolves to
# the freshly built tag. Cached rebuilds are ~10-30 s, full rebuilds ~3-4 min.
build_base_if_missing

# Non-interactive mode (CI) → skip prompts, write defaults if missing
if [ ! -t 0 ]; then
	if [ ! -f "$AUTH_FLAG" ]; then
		echo "ℹ Non-interactive: defaulting auth to standard."
		echo "standard" > "$AUTH_FLAG"
		[ -f "$MODE_FLAG" ] || echo "CLAUDE-dev.md" > "$MODE_FLAG"
	fi
	if [ ! -f "$FW_FLAG" ]; then
		echo "ℹ Non-interactive: defaulting firewall to strict."
		echo "strict" > "$FW_FLAG"
	fi
	sync_proxy_env "$(cat "$FW_FLAG" 2>/dev/null || echo strict)"
	print_summary
	exit 0
fi

echo ""
echo "=== DevContainer Setup ==="

# Each flag prompted independently — adding a new flag to an existing
# setup will trigger only its prompt at the next rebuild.
# Firewall has no prompt since A4 : default = strict (silent). Flip via
# firewall-mode.sh + rebuild.
[ ! -f "$AUTH_FLAG" ] && prompt_auth
[ ! -f "$MODE_FLAG" ] && prompt_claude_mode
[ ! -f "$FW_FLAG" ]   && write_firewall_default

# Idempotent re-sync : if the user manually edited firewall/default-mode
# between rebuilds, this aligns .env (HTTPS_PROXY etc) with the current flag.
sync_proxy_env "$(cat "$FW_FLAG" 2>/dev/null || echo strict)"

print_summary

# === Notify daemon (host-side, portable Node — runs while container is up) ===
# Spawned detached, survives initialize.sh exit. Self-exits when the container
# is gone (polls `docker ps` every 60 s). Idempotent — pid lockfile prevents
# double-spawn. Skipped silently if `node` is not on the host PATH.
#
# Diagnostic visibility : pre-flight prints the resolved node binary + paths,
# a spawn-boundary marker is appended to daemon.log, and ~1 s after the spawn
# we classify the outcome (claimed lockfile / bowed out / still booting /
# crashed silently). Catches the silent-crash failure mode where the previous
# "spawn attempted" message gave no insight into whether anything actually
# happened on disk.
spawn_notify_daemon() {
	local daemon_dir="$DEVCONTAINER_DIR/notify"
	local queue_dir="$DEVCONTAINER_DIR/notify/queue"
	local logfile="$queue_dir/daemon.log"
	local pidfile="$queue_dir/.daemon.pid"

	[ -f "$daemon_dir/index.js" ] || return 0
	mkdir -p "$queue_dir"

	local node_bin
	node_bin=$(command -v node 2>/dev/null) || {
		echo "ℹ Notify daemon : node not found on host PATH — skipping (install Node.js to enable desktop notifs)"
		return 0
	}

	# Pre-flight visibility — written to the INIT log (terminal/tee).
	echo "ℹ Notify daemon : node=$node_bin"
	echo "ℹ Notify daemon : entrypoint=${daemon_dir#$DEVCONTAINER_DIR/}/index.js"
	echo "ℹ Notify daemon : logfile=${logfile#$DEVCONTAINER_DIR/}"
	if [ ! -x "$node_bin" ]; then
		echo "⚠ Notify daemon : node binary not executable — skipping"
		return 0
	fi

	# Node 18+ requirement. The daemon uses numeric separators, optional
	# chaining, nullish coalescing, fs.rmSync etc. (lib/* files don't even
	# parse on Node < 14). Bail out cleanly with a clear message instead of
	# leaving a SyntaxError in daemon.log.
	local node_major
	node_major=$("$node_bin" --version 2>/dev/null | sed 's/^v//' | cut -d. -f1)
	if [ -n "$node_major" ] && [ "$node_major" -lt 18 ]; then
		echo "⚠ Notify daemon : node $("$node_bin" --version) too old (need 18+) — skipping"
		echo "  Upgrade via nvm: nvm install --lts && nvm use --lts"
		return 0
	fi

	# Mark the spawn boundary in daemon.log so we can correlate with whatever
	# the daemon writes (or doesn't write) after this point.
	printf '\n=== initialize.sh spawn attempt %s ===\n' "$(date -u +%FT%TZ)" >> "$logfile"

	# Wipe any residual status file from a previous run BEFORE spawn — otherwise
	# the polling loop below could read stale STATUS lines from a crashed daemon
	# that wrote its readback then died before cleanup. The new daemon will
	# rewrite this file (atomically) after its consumers init.
	rm -f "$queue_dir/.daemon.startup"

	# Run from PROJECT_DIR so the daemon's cwd-based auto-detect picks up
	# the right .devcontainer/notify/queue (queue dir is derived from cwd).
	#
	# The daemon owns its own lockfile (.daemon.pid) + liveness heartbeat —
	# re-spawn is idempotent. A second instance detects the first via the
	# pidfile (PID alive + mtime <= 30 s) and exits 0 silently. Stale zombies
	# get SIGKILL'd by the new instance on Unix hosts.
	pushd "$PROJECT_DIR" > /dev/null
	nohup "$node_bin" "$daemon_dir/index.js" "--launcher-pid=$PPID" >> "$logfile" 2>&1 &
	local new_pid=$!
	disown "$new_pid" 2>/dev/null || true
	popd > /dev/null

	# Post-spawn outcome classification. Give the daemon ~1 s to either claim
	# the lockfile, bow out via the existing-daemon guard, or crash.
	sleep 1
	local owner_pid=""
	[ -f "$pidfile" ] && owner_pid=$(cat "$pidfile" 2>/dev/null | tr -d '\n')
	if [ "$owner_pid" = "$new_pid" ]; then
		echo "✓ Notify daemon spawned (pid $new_pid owns lockfile, log: ${logfile#$DEVCONTAINER_DIR/})"
	elif [ -n "$owner_pid" ] && kill -0 "$owner_pid" 2>/dev/null; then
		echo "ℹ Notify daemon already running (pid $owner_pid) — attempt pid $new_pid exited cleanly"
	elif kill -0 "$new_pid" 2>/dev/null; then
		echo "ℹ Notify daemon : pid $new_pid still alive but no lockfile yet — may still be initializing (log: ${logfile#$DEVCONTAINER_DIR/})"
	else
		echo "⚠ Notify daemon : pid $new_pid gone, no lockfile claim — likely crashed silently."
		echo "  Last 20 lines of ${logfile#$DEVCONTAINER_DIR/} :"
		tail -20 "$logfile" 2>/dev/null | sed 's/^/    /'
		return 0
	fi

	# === Channel readback =====================================================
	# The daemon writes queue/.daemon.startup (atomic : tmp + rename) AFTER all
	# consumers' start() returned. Format is line-based, plain ASCII / Unicode :
	#   STATUS <name> <ok|skipped|fail> [k=v]...
	#   READY  pid=<n> channels=<csv-of-ok-channels>
	# We poll for ~3 s (100 ms × 30) then echo each line with an ANSI-colored
	# bracket glyph. Consumer init is synchronous and <100 ms in practice ; 3 s
	# covers the worst case (Linux sound probe across paplay/aplay/ffplay).
	local startup_file="$queue_dir/.daemon.startup"
	local waited=0
	while [ ! -f "$startup_file" ] && [ "$waited" -lt 30 ]; do
		sleep 0.1
		waited=$((waited + 1))
	done

	if [ ! -f "$startup_file" ]; then
		echo "⚠ Notify daemon : startup file absent après 3s — tail daemon.log :"
		tail -n 5 "$logfile" 2>/dev/null | sed 's/^/    /'
		return 0
	fi

	# Color guards : honor NO_COLOR (https://no-color.org). We CANNOT use
	# `[ -t 1 ]` here — initialize.sh:34 redirects stdout through tee, so fd 1
	# is always a pipe at this point (cf. comment at top of file). The original
	# terminal TTY-ness was captured pre-redirect on line 33 as
	# ORIG_STDOUT_TTY ; that's the signal `run_with_progress` uses too.
	local c_ok='' c_skip='' c_fail='' c_reset=''
	if [ -z "${NO_COLOR:-}" ] && [ "${ORIG_STDOUT_TTY:-0}" = "1" ]; then
		c_ok=$'\033[32m'   # green
		c_skip=$'\033[90m' # bright black / grey
		c_fail=$'\033[31m' # red
		c_reset=$'\033[0m'
	fi

	local line channels
	while IFS= read -r line; do
		case "$line" in
			"STATUS "*" ok "*|"STATUS "*" ok")
				printf '  %s[\xe2\x9c\x93]%s %s\n' "$c_ok"   "$c_reset" "${line#STATUS }" ;;
			"STATUS "*" skipped "*|"STATUS "*" skipped")
				printf '  %s[-]%s %s\n'            "$c_skip" "$c_reset" "${line#STATUS }" ;;
			"STATUS "*" fail "*|"STATUS "*" fail")
				printf '  %s[x]%s %s\n'            "$c_fail" "$c_reset" "${line#STATUS }" ;;
			"READY "*)
				channels="${line#*channels=}"
				channels="${channels%% *}"
				echo "ℹ Notify daemon : channels=$channels" ;;
		esac
	done < "$startup_file"
}
spawn_notify_daemon || true

# Pause only when an interactive prompt actually ran. prompt_auth is now a
# silent info banner (no input), so only prompt_claude_mode (which sets
# CLAUDE_MODE via `read`) should keep the user on the screen.
if [ -n "${CLAUDE_MODE:-}" ]; then
	read -p "Press Enter to continue..." _ || true
fi
