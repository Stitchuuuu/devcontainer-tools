# Show post-start log path on first shell
# Sourced from .zshrc/.bashrc at container startup
# Only run in interactive terminals
if [[ $- == *i* ]] && [ -f /tmp/post-start.log ]; then
  echo "📄 Post-start log: /tmp/post-start.log"
fi

# Credentials conflict resolution
if [[ $- == *i* ]] && [ -f /tmp/.claude-creds-conflict ]; then
  echo ""
  echo "⚠️  Claude credentials conflict detected!"
  echo "  Both local and shared volumes have valid but different tokens."
  echo ""
  echo "  [1] Keep local  (this container's token)"
  echo "  [2] Keep shared (from another project/session)"
  echo ""
  read -p "Choose [1/2]: " CRED_CHOICE
  LOCAL_CRED="/home/node/.claude/.credentials.json"
  SHARED_CRED="/home/node/.claude-creds/.credentials.json"
  if [ "$CRED_CHOICE" = "2" ]; then
    cp "$SHARED_CRED" "$LOCAL_CRED"
    chmod 600 "$LOCAL_CRED"
    echo "✓ Using shared token."
  else
    cp "$LOCAL_CRED" "$SHARED_CRED"
    echo "✓ Using local token."
  fi
  rm -f /tmp/.claude-creds-conflict
fi

# Auto-sync credentials on terminal open (catches Claude Code token refreshes)
if [[ $- == *i* ]] && [ -x /workspace/.devcontainer/claude/sync-creds.sh ]; then
  VERBOSE=1 /workspace/.devcontainer/claude/sync-creds.sh
fi

# GitHub CLI auto-auth (only when not already authenticated — e.g. standard mode)
# Require a real TTY ([ -t 0 ]) — VS Code's resolveShellPath spawns `zsh -lic`
# which IS interactive ($- has 'i') but has NO TTY, and would block on the
# gh auth login device flow, causing a 10s spawnSync timeout in the extension.
if [[ $- == *i* ]] && [ -t 0 ] && ! gh auth status &>/dev/null; then
  echo ""
  echo "GitHub CLI is not authenticated. Starting login..."
  echo ""
  gh auth login
  if gh auth status &>/dev/null; then
    gh auth setup-git
    echo ""
    echo "Git credential helper configured."
  fi
fi

# ⚠️ Warn if test-root still has sudo access (sudoers entry is the real risk)
if [[ $- == *i* ]] && sudo -l 2>/dev/null | grep -q "test-root"; then
  echo ""
  echo "⚠️  WARNING: test-root.sh has sudo access!"
  echo "⚠️  Remove the test RUN line in .devcontainer/Dockerfile before committing."
fi

# mitmproxy CA env vars for tools that don't read the system trust store
# (Python requests, Node fetches not via system, …). HTTPS_PROXY is already
# set by docker-compose env_file (PID 1 inherits .env automatically), so we
# only handle CA-bundle paths here. Guard on cert presence — no-op in basic.
if [ -f /var/lib/mitmproxy/mitmproxy-ca-cert.pem ]; then
  export REQUESTS_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt
  export SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
  export GIT_SSL_CAINFO=/etc/ssl/certs/ca-certificates.crt
fi

# Fallback init of ~/.claude-local for the "Reload Window after claude-switch"
# flow. post-start.sh is the canonical place that creates this dir (with the
# shared symlinks to ~/.claude/{commands,skills,memory,plugins,settings.json,
# .claude.json}), but post-start only fires at container start — so the very
# first time the user toggles cloud→local without rebuilding, the dir is
# missing and CLAUDE_CONFIG_DIR points at nothing. We re-create it here on
# shell open as a safety net : idempotent (no-op if already exists), zero
# cost in cloud mode (the grep returns 0 lines and the block skips).
if grep -qE '^ANTHROPIC_BASE_URL=http://ollama\.(internal|local)' /workspace/.devcontainer/.env 2>/dev/null \
   && [ ! -d "$HOME/.claude-local" ]; then
  mkdir -p "$HOME/.claude-local" && chmod 700 "$HOME/.claude-local"
  if [ -d "$HOME/.claude" ]; then
    for _claude_local_path in commands skills memory plugins settings.json .claude.json; do
      [ -e "$HOME/.claude/$_claude_local_path" ] && \
        ln -sfn "$HOME/.claude/$_claude_local_path" "$HOME/.claude-local/$_claude_local_path"
    done
    unset _claude_local_path
  fi
  printf '\033[1;36mℹ️  ~/.claude-local initialized via shell-init fallback (Reload Window without Rebuild)\033[0m\n'
fi

# Session summary
if [[ $- == *i* ]]; then
  AUTH_MODE="gh token only"

  # Detect Claude mode
  CLAUDE_MODE="dev"
  if [ -f /workspace/.devcontainer/.configured-claude-mode ]; then
    CLAUDE_MODE=$(cat /workspace/.devcontainer/.configured-claude-mode | sed 's/CLAUDE-//;s/\.md//')
  fi

  # Detect firewall mode (canonical: strict / basic / off — aliases paranoid / okeish accepted)
  FW_MODE="strict"
  [ -f /workspace/.devcontainer/.configured-firewall-mode ] && \
    FW_MODE=$(cat /workspace/.devcontainer/.configured-firewall-mode)

  # Local overrides suffix — appended to the "Firewall:" line so the user
  # sees at every shell launch whether they're running a customised policy.
  LOCAL_TXT=/workspace/.devcontainer/firewall/domains.local.txt
  LOCAL_D=/workspace/.devcontainer/firewall/policy.local.d
  # grep -c prints "0" on no match but exits 1 — `|| true` allows that without
  # re-emitting "0" (would give "0\n0" multi-line and break the -gt below).
  LOCAL_HOSTS=$(grep -cE "^[[:space:]]*[^#[:space:]]" "$LOCAL_TXT" 2>/dev/null || true)
  LOCAL_HOSTS="${LOCAL_HOSTS:-0}"
  LOCAL_POLICY=0
  [ -d "$LOCAL_D" ] && LOCAL_POLICY=$(find "$LOCAL_D" -maxdepth 1 -name "*.yaml" -type f 2>/dev/null | wc -l | tr -d ' ')
  OVERRIDES_SUFFIX=""
  if [ "${LOCAL_HOSTS:-0}" -gt 0 ] || [ "${LOCAL_POLICY:-0}" -gt 0 ]; then
    OVERRIDES_SUFFIX=" ($LOCAL_HOSTS host + $LOCAL_POLICY policy local overrides)"
  fi

  echo ""
  echo "──────────────────────────────────"
  echo "  GitHub:    $AUTH_MODE"
  echo "  Claude:    $CLAUDE_MODE"
  # v2.1-2 — show whether the Claude binary is the extension symlink (Phase B)
  # or an npm install fallback. Sentinel /etc/claude-fallback-warn is touched
  # by Dockerfile.base when Phase B did NOT take effect.
  if [ -f /etc/claude-fallback-warn ]; then
    printf '\033[1;33m  Binary:    npm fallback (Phase B failed — cat /etc/claude-source)\033[0m\n'
  elif [ -r /etc/claude-source ]; then
    SRC=$(cat /etc/claude-source)
    case "$SRC" in
      extension:*) echo "  Binary:    extension (Phase B)" ;;
    esac
  fi
  echo "  Firewall:  $FW_MODE$OVERRIDES_SUFFIX"

  # A2 blocks-log summary: only meaningful in strict mode (the addons that
  # write to /var/log/mitmproxy-blocks.log only run there). Differentiate
  # blocked (B) from warn-only (W) so the user sees if they're running in
  # audit mode (lots of W, no B). Fast grep — runs on every shell.
  case "$FW_MODE" in
    strict|paranoid)
      if [ -r /var/log/mitmproxy-blocks.log ]; then
        # grep -c prints "0" then exits 1 on no match — `|| true` swallows
        # the exit so we don't get "0\n0" multi-line. Same fix as line 88-91.
        TOTAL=$(wc -l < /var/log/mitmproxy-blocks.log 2>/dev/null || true)
        TOTAL="${TOTAL:-0}"
        if [ "$TOTAL" -gt 0 ] 2>/dev/null; then
          WARNS=$(grep -c '"mode":"warn"' /var/log/mitmproxy-blocks.log 2>/dev/null || true)
          WARNS="${WARNS:-0}"
          BLOCKED=$((TOTAL - WARNS))
          echo "  Events:    $BLOCKED blocked / $WARNS warn-only — run 'firewall-blocks' to inspect"
        fi
      fi
      ;;
  esac

  # Scan-deps reminder (F2) — 1-line cyan passive prompt if any package.json
  # is newer than its corresponding `domains.d/npm.txt`. Mirrors the loud
  # yellow ASCII box shown once per container start in post-start.sh — but
  # quieter, refreshed every shell open.
  if command -v python3 >/dev/null 2>&1; then
    python3 - <<'PY' 2>/dev/null || true
import os, subprocess
DOMAINS_D = '/workspace/.devcontainer/firewall/domains.d'
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
if not os.path.exists(target):
    n_changed = len(manifests)
    msg = f"{n_changed} manifest(s) detected, never extracted"
else:
    target_mt = os.path.getmtime(target)
    n_changed = sum(1 for m in manifests if os.path.getmtime(m) > target_mt)
    if not n_changed:
        raise SystemExit
    msg = f"{n_changed} manifest(s) modified since last extract"
print(f"\033[1;36m  Scan-deps: {msg} — run extract-auto-dependencies\033[0m")
PY
  fi

  echo "──────────────────────────────────"
  echo "  Flip firewall mode (then rebuild) :"
  echo "    .devcontainer/firewall-mode.sh {strict|basic|off}"
  echo "  Reconfigure other flags :"
  echo "    rm .devcontainer/.configured-auth            # reset GitHub auth"
  echo "    rm .devcontainer/.configured-claude-mode     # reset Claude mode"
  echo "    rm .devcontainer/.configured-firewall-mode   # reset firewall mode"
  echo "  Then rebuild the container."
  echo "──────────────────────────────────"
fi

# Claude Code local/cloud mode is switched from the HOST, not the container —
# see .devcontainer/host-helpers/claude-switch and knowledge/ollama-local.md. Keeping
# the toggle outside the container means a compromised in-container process
# can't silently flip the LLM endpoint. The ~/.claude-local/ isolation
# directory is initialized by post-start.sh when local mode is detected in
# .env at container boot.
