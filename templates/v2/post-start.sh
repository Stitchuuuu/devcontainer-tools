#!/bin/bash
# Post-start script for devcontainer
# Runs at each container start (postStartCommand)

# === Lifecycle logging === (mirror initialize.sh — see comment block there).
# Always-on : .log via tee. DEBUG=1 only : .trace (xtrace).
# This hook runs at every container start, so it's also the natural place to
# gate disk usage in .devcontainer/logs/ — drop hook logs older than 7 days.
mkdir -p /workspace/.devcontainer/logs 2>/dev/null || true
find /workspace/.devcontainer/logs -maxdepth 1 -type f \
    \( -name '*.log' -o -name '*.trace' \) -mtime +7 -delete 2>/dev/null || true
TS=$(date +%Y%m%d-%H%M%S)
LOG=/workspace/.devcontainer/logs/post-start-${TS}.log
TRACE=/workspace/.devcontainer/logs/post-start-${TS}.trace
exec > >(tee -a "$LOG") 2>&1
if [ "${DEBUG:-0}" = "1" ]; then
  exec 19>>"$TRACE"
  export BASH_XTRACEFD=19
  PS4='+ ${BASH_SOURCE##*/}:${LINENO}: '
  set -x
  TRACE_LOC="${TRACE#/workspace/}"
else
  rm -f "$TRACE"
  TRACE_LOC="(disabled — set DEBUG=1 in .env to enable xtrace)"
fi
echo "=== post-start $(date) ==="
echo "  log:   ${LOG#/workspace/}"
echo "  trace: $TRACE_LOC"

# Wipe the Claude Code VS Code extension observation/control JSONLs at every
# container start. Observation/audit + control-channel data — no value retaining
# across boots. The JS appendFile in the user-action-observer patch creates
# inbound.jsonl on the first event ; outbound-action-injector's watcher creates
# outbound.jsonl + pending-perms.jsonl at ext startup. Parent dir
# (.devcontainer/logs) is created above by the lifecycle block.
rm -f /workspace/.devcontainer/logs/claude-code-vscode-ext-inbound.jsonl \
      /workspace/.devcontainer/logs/claude-code-vscode-ext-outbound.jsonl \
      /workspace/.devcontainer/logs/claude-code-vscode-ext-pending-perms.jsonl

# Hide bind-mounted .vscode/settings.json from git
# (the devcontainer overrides it via docker-compose volume mount)
git -C /workspace update-index --skip-worktree .vscode/settings.json 2>/dev/null || true

# Firewall mode — single source of truth since bake-only : the baked file
# /etc/devcontainer-firewall/default-mode (read by init-firewall.sh too).
# Used here only for the BASIC-mode banner below.
FW_MODE=$(cat /etc/devcontainer-firewall/default-mode 2>/dev/null | tr -d '[:space:]')
FW_MODE="${FW_MODE:-strict}"

# Always invoke — init-firewall.sh has its own kernel-state guard (skips when
# firewall is already up). Self-healing on restart (netns wiped → re-init).
FW_DEBUG_ARG=""
[ "${CLAUDE_CODE_FIREWALL_DEBUG:-}" = "true" ] && FW_DEBUG_ARG="--debug"
sudo /usr/local/bin/init-firewall.sh $FW_DEBUG_ARG
# init-firewall.sh handles HTTPS_PROXY propagation (writes /etc/environment +
# /etc/profile.d/devcontainer-proxy.sh in strict mode, cleans up otherwise).

# Loud banner if the user is running in basic mode (A4 default is strict).
# strict mode = silent — the safe default doesn't need a banner.
case "$FW_MODE" in
  basic|okeish)
    printf '\033[1;33m'
    cat <<'BANNER'
╔════════════════════════════════════════════════════════════════╗
║  ⚠  Firewall in BASIC mode (DNS allowlist only, no L7 filter)  ║
║                                                                ║
║  To re-enable strict :                                         ║
║     .devcontainer/firewall-mode.sh strict   [host or container]║
║     Rebuild the container in VS Code                           ║
╚════════════════════════════════════════════════════════════════╝
BANNER
    printf '\033[0m\n'
    ;;
esac

# 1-line info banner if local overrides are active (committed policy is augmented
# locally — useful to flag for visibility, no enforcement implication).
LOCAL_TXT=/workspace/.devcontainer/firewall/domains.local.txt
LOCAL_D=/workspace/.devcontainer/firewall/policy.local.d
# grep -c prints "0" on no match but exits 1 — `|| true` allows that without
# re-emitting "0" (would give "0\n0" multi-line and break the -gt below).
LOCAL_HOSTS=$(grep -cE "^[[:space:]]*[^#[:space:]]" "$LOCAL_TXT" 2>/dev/null || true)
LOCAL_HOSTS="${LOCAL_HOSTS:-0}"
LOCAL_POLICY=0
[ -d "$LOCAL_D" ] && LOCAL_POLICY=$(find "$LOCAL_D" -maxdepth 1 -name "*.yaml" -type f 2>/dev/null | wc -l | tr -d ' ')
if [ "${LOCAL_HOSTS:-0}" -gt 0 ] || [ "${LOCAL_POLICY:-0}" -gt 0 ]; then
  printf '\033[1;36mℹ️  Firewall local overrides active: %s extra host(s) + %s policy.local.d file(s)\033[0m\n' \
    "$LOCAL_HOSTS" "$LOCAL_POLICY"
fi

# Claude Code local mode banner + isolation init (cf. .devcontainer/knowledge/ollama-local.md).
# Anchored regex matches ONLY the uncommented active line — so the banner
# disappears automatically when host-helpers/claude-switch cloud re-comments
# the var. When local mode is active, ensure ~/.claude-local/ exists with the
# right symlinks (shared skills/commands/memory, isolated creds/projects/todos)
# before any shell runs `claude`. Doing the init here rather than in
# shell-init.sh keeps it a one-shot per container start instead of every shell.
CLAUDE_LOCAL_DIR=/home/node/.claude-local
_init_claude_local_dir() {
  [ -d "$CLAUDE_LOCAL_DIR" ] && return 0
  mkdir -p "$CLAUDE_LOCAL_DIR" && chmod 700 "$CLAUDE_LOCAL_DIR"
  if [ -d "$HOME/.claude" ]; then
    for path in commands skills memory plugins settings.json .claude.json; do
      if [ -e "$HOME/.claude/$path" ]; then
        ln -sfn "$HOME/.claude/$path" "$CLAUDE_LOCAL_DIR/$path"
      fi
    done
  fi
  printf '\033[1;36mℹ️  Initialized %s (shared skills/commands/memory, isolated creds)\033[0m\n' "$CLAUDE_LOCAL_DIR"
}

if grep -qE '^ANTHROPIC_BASE_URL=http://ollama\.internal' /workspace/.devcontainer/.env 2>/dev/null; then
  printf '\033[1;33m🦙 Claude mode: LOCAL (ollama.internal:11434 — via mitmproxy audit)\033[0m\n'
  _init_claude_local_dir
elif grep -qE '^ANTHROPIC_BASE_URL=http://ollama\.local' /workspace/.devcontainer/.env 2>/dev/null; then
  printf '\033[1;31m🦙 Claude mode: LOCAL BYPASS (ollama.local:11434 — NO audit, debug only)\033[0m\n'
  _init_claude_local_dir
fi

# Claude binary fallback sentinel (v2.1-2). /etc/claude-fallback-warn is touched
# by Dockerfile.base when Phase B (symlink to extension's embedded binary) was
# NOT used — either because the VSIX download failed at build, or because the
# extracted extension had no usable binary at the expected path.
# /etc/claude-source carries the human-readable detail.
if [ -f /etc/claude-fallback-warn ]; then
  SRC=$(cat /etc/claude-source 2>/dev/null || echo unknown)
  printf '\033[1;33m'
  printf '╔════════════════════════════════════════════════════════════════╗\n'
  printf '║  ⚠  Claude binary: npm fallback active (Phase B failed)        ║\n'
  printf '║     source: %-51s║\n' "${SRC:0:51}"
  printf '║                                                                ║\n'
  printf '║  Image is ~224 MB heavier than Phase B target. Investigate at  ║\n'
  printf '║  next CLAUDE_CODE_VERSION bump :                               ║\n'
  printf '║    - cat /etc/claude-source        (which branch fired)        ║\n'
  printf '║    - ls /home/node/.vscode-server/extensions/anthropic.claude* ║\n'
  printf '║    - readlink -f /usr/local/bin/claude                         ║\n'
  printf '║  See LOG.md v2.1-2 "Failsafe troubleshooting" section.         ║\n'
  printf '╚════════════════════════════════════════════════════════════════╝\n'
  printf '\033[0m'
fi

# Claude Code update probe (v2.1-1). registry.npmjs.org/@anthropic-ai/claude-code
# is GET-allowed by policy.d/registry.npmjs.org.yaml — same channel npm itself uses.
# Silent if registry unreachable (off/basic-without-domain, network down) or already up to date.
check_claude_update() {
  command -v claude >/dev/null 2>&1 || return 0
  command -v curl   >/dev/null 2>&1 || return 0
  command -v python3 >/dev/null 2>&1 || return 0
  local installed latest
  installed=$(claude --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
  [ -z "$installed" ] && return 0
  latest=$(curl -fsSL --max-time 5 \
    https://registry.npmjs.org/@anthropic-ai/claude-code 2>/dev/null \
    | python3 -c "import json,sys;print(json.load(sys.stdin)['dist-tags']['latest'])" 2>/dev/null)
  [ -z "$latest" ] && return 0
  [ "$installed" = "$latest" ] && return 0
  printf '\033[1;33m'
  printf '╔════════════════════════════════════════════════════════════════╗\n'
  printf '║  ⚠  Claude Code update available                               ║\n'
  printf '║     installed: %-48s║\n' "$installed"
  printf '║     latest:    %-48s║\n' "$latest"
  printf '║                                                                ║\n'
  printf '║  Bump CLAUDE_CODE_VERSION in .devcontainer/.env then           ║\n'
  printf '║  "Dev Containers: Rebuild Container" in VS Code.               ║\n'
  printf '╚════════════════════════════════════════════════════════════════╝\n'
  printf '\033[0m'
}
check_claude_update

# Scan-deps boot hook (F2) — banner if project manifests changed since the
# last `extract-auto-dependencies` run. Compares per-manifest mtime against
# the matching `domains.d/<eco>.txt` mtime (the deterministic allowlist file
# generated by extract-auto). If stale, suggest re-running the extractor.
if [ -z "${SCAN_DEPS_HOOK_DISABLED:-}" ] && command -v python3 >/dev/null 2>&1; then
  python3 - <<'PY' || true
import os, subprocess
DOMAINS_D = '/workspace/.devcontainer/firewall/domains.d'
# F2: package.json → domains.d/npm.txt is the only auto-extracted ecosystem.
# composer.json / pyproject.toml / requirements.txt / Cargo.toml / go.mod still
# use /scan-deps — their staleness is handled by the skill, not this banner.
MANIFEST_NAME = 'package.json'
ECO_FILE = 'npm.txt'
def find_manifests(depths=(3, 5, 8, 10)):
    for d in depths:
        res = subprocess.run(
            ['find', '/workspace', '-maxdepth', str(d), '-type', 'f',
             '-name', MANIFEST_NAME,
             '-not', '-path', '*/node_modules/*',
             '-not', '-path', '*/vendor/*',
             '-not', '-path', '*/.git/*',
             '-not', '-path', '*/__pycache__/*',
             '-not', '-path', '*/research-bundles/*'],
            capture_output=True, text=True)
        ms = [p for p in res.stdout.splitlines() if p]
        if ms:
            return ms
    return []
manifests = find_manifests()
if not manifests:
    raise SystemExit
target = os.path.join(DOMAINS_D, ECO_FILE)
stale = False
if not os.path.exists(target):
    stale = True
else:
    try:
        target_mt = os.path.getmtime(target)
    except OSError:
        stale = True
    else:
        for m in manifests:
            try:
                if os.path.getmtime(m) > target_mt:
                    stale = True
                    break
            except OSError:
                continue
if stale:
    yellow = '\033[1;33m'
    reset = '\033[0m'
    lines = [
        '╔════════════════════════════════════════════════════════════════╗',
        '║  ⚠  Project manifests changed since last firewall extract      ║',
        '║                                                                ║',
        '║  Refresh the allowlist (deterministic, no AI) :                ║',
        '║     bash .devcontainer/skills/scan-deps/                       ║',
        '║          extract-auto-dependencies                             ║',
        '║                                                                ║',
        '║  Then reload the firewall :                                    ║',
        '║     sudo /usr/local/bin/init-firewall.sh                       ║',
        '║                                                                ║',
        '║  Or run /scan-deps in Claude for review + edge cases.          ║',
        '╚════════════════════════════════════════════════════════════════╝',
    ]
    print(yellow + '\n'.join(lines) + reset)
PY
fi

# Claude account data: sync between shared volume and per-container config
SHARED_DIR="/home/node/.claude-creds"
LOCAL_DIR="/home/node/.claude"

# .credentials.json — sync delegated to dedicated script (reused by shell-init + Claude hooks)
SYNC_CREDS="/workspace/.devcontainer/claude/sync-creds.sh"
if [ -x "$SYNC_CREDS" ]; then
  VERBOSE=1 "$SYNC_CREDS"
else
  echo "⚠️  $SYNC_CREDS missing — skipping credentials sync."
fi

# .claude.json — sync by file timestamp (settings/flags, no expiry)
CONF=".claude.json"
if [ -f "$SHARED_DIR/$CONF" ] && [ ! -f "$LOCAL_DIR/$CONF" ]; then
  cp "$SHARED_DIR/$CONF" "$LOCAL_DIR/$CONF"
  chmod 600 "$LOCAL_DIR/$CONF"
  echo "✓ $CONF restored from shared volume."
elif [ -f "$LOCAL_DIR/$CONF" ] && [ ! -f "$SHARED_DIR/$CONF" ]; then
  cp "$LOCAL_DIR/$CONF" "$SHARED_DIR/$CONF"
  echo "✓ $CONF saved to shared volume."
elif [ -f "$LOCAL_DIR/$CONF" ] && [ -f "$SHARED_DIR/$CONF" ]; then
  if [ "$LOCAL_DIR/$CONF" -nt "$SHARED_DIR/$CONF" ]; then
    cp "$LOCAL_DIR/$CONF" "$SHARED_DIR/$CONF"
    echo "✓ $CONF updated in shared volume."
  else
    cp "$SHARED_DIR/$CONF" "$LOCAL_DIR/$CONF"
    chmod 600 "$LOCAL_DIR/$CONF"
    echo "✓ $CONF restored from shared volume (newer)."
  fi
fi

# Pre-configure Claude CLI to skip onboarding wizard (theme + completed flag)
CLAUDE_JSON="$LOCAL_DIR/.claude.json"
if [ -f "$CLAUDE_JSON" ]; then
  python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
changed = False
if not d.get('hasCompletedOnboarding'):
    d['hasCompletedOnboarding'] = True
    changed = True
if not d.get('theme'):
    d['theme'] = 'dark'
    changed = True
if changed:
    with open(sys.argv[1], 'w') as f:
        json.dump(d, f)
    print('✓ Claude CLI pre-configured (onboarding + theme).')
" "$CLAUDE_JSON"
fi

# GitHub CLI authentication — user runs `gh auth login` manually in a terminal
if gh auth status &>/dev/null; then
  gh auth setup-git 2>/dev/null
  echo "✓ Git configured with GitHub CLI credentials."
else
  echo ""
  echo "⚠️  ========================================"
  echo "⚠️  GitHub CLI is NOT authenticated!"
  echo "⚠️  Git push/pull will not work."
  echo "⚠️  Open a terminal and run: gh auth login"
  echo "⚠️  ========================================"
  echo ""
fi

# Skills — sync skill commands and hooks
if [ -f /workspace/.devcontainer/skills/sync-skills.sh ]; then
  bash /workspace/.devcontainer/skills/sync-skills.sh
fi

# Watch-log cleanup — drop pending/* > 60 min stale (skill /watch-log, C)
CLEANUP=/workspace/.devcontainer/host-helpers/watch-log-cleanup
[ -x "$CLEANUP" ] && "$CLEANUP"

# Merge creds-sync hooks (Stop + SessionEnd) into ~/.claude/settings.json
# so the shared volume stays fresh whenever Claude Code refreshes the OAuth token
# during an active session. Idempotent — dedup by command.
SETTINGS="$LOCAL_DIR/settings.json"
SYNC_CREDS_CMD="sh /workspace/.devcontainer/claude/sync-creds.sh"
if [ -x "$SYNC_CREDS" ] && command -v python3 >/dev/null 2>&1; then
  mkdir -p "$(dirname "$SETTINGS")"
  [ -f "$SETTINGS" ] || echo '{}' > "$SETTINGS"
  python3 -c "
import json, sys
path, cmd = sys.argv[1], sys.argv[2]
with open(path) as f:
    s = json.load(f)
hooks = s.setdefault('hooks', {})
changed = False
for event in ('Stop', 'SessionEnd'):
    entries = hooks.setdefault(event, [])
    seen = set()
    for entry in entries:
        for h in entry.get('hooks', []):
            if 'command' in h:
                seen.add(h['command'])
    if cmd not in seen:
        entries.append({'matcher': '', 'hooks': [{'type': 'command', 'command': cmd}]})
        changed = True
if changed:
    with open(path, 'w') as f:
        json.dump(s, f, indent=2)
    print('✓ creds-sync hooks merged into settings.json')
else:
    print('✓ creds-sync hooks already registered')
" "$SETTINGS" "$SYNC_CREDS_CMD"
fi

# Safety net : if vscode-server raced ahead of the firewall init and some
# extensions failed to download with ECONNREFUSED, re-install them now that
# mitmproxy is confirmed up. Idempotent — already-installed ones are skipped.
INSTALL_EXTS=/workspace/.devcontainer/install-extensions.sh
if [ -x "$INSTALL_EXTS" ]; then
  if ! command -v code >/dev/null 2>&1; then
    # vscode-server layouts vary by version and arch :
    #   /vscode/vscode-server/bin/<arch>/<hash>/bin/remote-cli/code   (recent)
    #   /vscode/vscode-server/bin/<hash>/bin/remote-cli/code          (older)
    #   $HOME/.vscode-server/bin/<hash>/bin/remote-cli/code           (legacy)
    CODE_BIN=$(find /vscode/vscode-server "$HOME/.vscode-server" \
               -maxdepth 6 -type f -name code -path '*remote-cli*' \
               2>/dev/null | head -1)
    [ -n "$CODE_BIN" ] && export PATH="$(dirname "$CODE_BIN"):$PATH"
  fi
  if command -v code >/dev/null 2>&1; then
    "$INSTALL_EXTS" || true
  else
    echo "ℹ 'code' CLI not yet available — skipping extension safety net"
  fi
fi

