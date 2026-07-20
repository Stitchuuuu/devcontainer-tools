#!/usr/bin/env bash
# Diagnose `claude --print 'ping'` behavior in local Ollama mode AND in
# local-proxy mode (sidecar UniClaudeProxy, see knowledge/ollama-local.md § Mode
# local-proxy). Probes the Ollama direct path AND the claude-bridge sidecar
# path (audited + .local bypass), so the same script covers both modes.
#
# Run this INSIDE the container, after switching modes from the host
# (`bash .devcontainer/host-helpers/claude-switch {local|local-proxy}` +
# Rebuild Container) and confirming Ollama runs on the Mac. The script
# collects everything we need to root-cause hangs / translation issues
# into a single log file under the bind-mounted .devcontainer/logs/.
#
# It runs the SAME `claude --print 'ping'` twice : once with the active
# (local or local-proxy) env, once with the local-mode vars explicitly
# unset so claude falls back to cloud. Comparing the two outputs tells
# us whether the bug is in the claude binary itself or in the local
# wiring.
#
# Usage : bash .devcontainer/diag-ollama-local.sh
set -uo pipefail

# Guard : this script tests behavior FROM INSIDE the container. Running it on
# the host would hit nonexistent paths (/workspace, /var/log/mitmproxy-*)
# and produce a useless empty log.
if [ ! -f /.dockerenv ] && [ -z "${REMOTE_CONTAINERS:-}" ]; then
  echo "ERROR: This script must run INSIDE the container (it tests container-local state)." >&2
  echo "       Open a VSCode terminal inside the devcontainer, then run :" >&2
  echo "         bash .devcontainer/diag-ollama-local.sh" >&2
  exit 2
fi

TS=$(date +%Y%m%d-%H%M%S)
LOG=/workspace/.devcontainer/logs/diag-ollama-local-${TS}.log
exec > >(tee -a "$LOG") 2>&1

hr() { printf '\n──── %s ────\n' "$*"; }

echo "=== diag-ollama-local $(date) ==="
echo "host  : $(hostname)  (in-container: $([ -f /.dockerenv ] && echo yes || echo no))"
echo "log   : ${LOG#/workspace/}"

hr "1. .env routing state"
grep -nE "^(# *)?(ANTHROPIC_|CLAUDE_CONFIG_DIR|CLAUDE_CODE_FIREWALL_ALLOWED)" \
    /workspace/.devcontainer/.env 2>/dev/null \
    || echo "(no matching vars found)"

hr "2. Current shell env (what THIS shell sees)"
env | grep -E "^(ANTHROPIC_|CLAUDE_CONFIG|HTTP_PROXY|HTTPS_PROXY|NO_PROXY)=" \
    | sort \
    || echo "(no matching env vars)"

hr "3. claude binary resolution"
type claude 2>&1 | head -5
echo "PATH : $PATH"

hr "4. ~/.claude-local state"
if [ -d "$HOME/.claude-local" ]; then
  ls -la "$HOME/.claude-local" | head -20
  echo "(symlinks resolved : )"
  ls -laL "$HOME/.claude-local" 2>&1 | head -10
else
  echo "✗ $HOME/.claude-local does NOT exist (post-start.sh did not init it)"
  echo "  → confirms hang cause if CLAUDE_CONFIG_DIR points there"
fi

hr "5. DNS resolution of ollama.internal + ollama.local"
getent hosts ollama.internal 2>&1 || echo "(getent failed)"
getent hosts ollama.local    2>&1 || echo "(getent failed — bypass alias absent)"

hr "5b. DNS resolution of claude-bridge + claude-bridge.local"
echo "(audited path goes through mitm ; .local bypass goes direct TCP)"
getent hosts claude-bridge       2>&1 || echo "(getent failed — sidecar service not declared in compose, or dnsmasq not forwarding)"
getent hosts claude-bridge.local 2>&1 || echo "(getent failed — CNAME alias not set in init-firewall.sh)"

hr "6. TCP reachability to ollama.internal:11434 (direct, bypass proxy)"
timeout 5 bash -c 'cat < /dev/tcp/ollama.internal/11434' 2>&1 \
  | head -3 || echo "(TCP test timed out or failed — port closed / Ollama not running)"

hr "6b. TCP reachability to claude-bridge:9223 (audited) + claude-bridge.local:9223 (bypass)"
echo "audited :"
timeout 5 bash -c 'cat < /dev/tcp/claude-bridge/9223' 2>&1 \
  | head -3 || echo "(TCP test failed — sidecar down? Run from host : bash .devcontainer/host-helpers/claude-bridge status)"
echo "bypass  :"
timeout 5 bash -c 'cat < /dev/tcp/claude-bridge.local/9223' 2>&1 \
  | head -3 || echo "(TCP test failed — .local alias not resolving, see 5b)"

hr "7. /api/version via mitmproxy (HTTPS_PROXY=$HTTPS_PROXY)"
curl -sS --max-time 5 -i http://ollama.internal:11434/api/version 2>&1 | head -20

hr "8. /api/tags via mitmproxy (lists locally pulled models)"
curl -sS --max-time 5 http://ollama.internal:11434/api/tags 2>&1 \
  | head -50

# Common payload for both 9a (ollama) and 9b (cloud) — same prompt, same
# model name, same max_tokens. Lets you eyeball the two raw responses
# side by side and judge whether Ollama is returning sensible content.
PAYLOAD='{
  "model": "claude-opus-4-7",
  "max_tokens": 32,
  "messages": [{"role": "user", "content": "ping"}]
}'

hr "9a. /v1/messages POST → ollama.internal (raw API response from Ollama)"
curl -sS --max-time 30 -i \
  -X POST http://ollama.internal:11434/v1/messages \
  -H "Content-Type: application/json" \
  -H "anthropic-version: 2023-06-01" \
  -H "x-api-key: ollama" \
  -d "$PAYLOAD" 2>&1 | head -80

hr "9b. /v1/messages POST → api.anthropic.com (raw API response from cloud — control)"
if [ -n "${ANTHROPIC_API_KEY:-}" ]; then
  # NOTE : the API key is sent in the request header but NOT printed to the
  # log — curl only echoes the response body + headers, not the request.
  curl -sS --max-time 30 -i \
    -X POST https://api.anthropic.com/v1/messages \
    -H "Content-Type: application/json" \
    -H "anthropic-version: 2023-06-01" \
    -H "x-api-key: $ANTHROPIC_API_KEY" \
    -d "$PAYLOAD" 2>&1 | head -80
else
  cat <<'SKIP'
(skipped — no ANTHROPIC_API_KEY in env)

Why : Claude Max uses OAuth, not a standalone API key. The OAuth token
lives in ~/.claude/.credentials.json but extracting + printing it into
a shared log file is risky (the log goes into .devcontainer/logs/, which
is bind-mounted and gitignored but might still be pasted around).

To run this section once, grab an API key from
https://console.anthropic.com/settings/keys and inject it inline :

  ANTHROPIC_API_KEY=sk-ant-... bash .devcontainer/diag-ollama-local.sh

The key won't be saved anywhere — it lives in the env for that one
invocation and disappears.

Alternative : compare 9a (Ollama raw) against 10b (claude --print cloud).
Not apples-to-apples (10b is filtered through the Claude agent system
prompt) but tells you whether cloud at least returns coherent text.
SKIP
fi

hr "9c. /v1/messages POST → claude-bridge:9223 (audited, sidecar-translated)"
echo "Same payload as 9a/9b ; sidecar translates <think> blocks → Anthropic 'thinking'"
echo "(stream:false here for clean JSON ; non-stream path keeps text only — see knowledge/ollama-local.md)"
curl -sS --max-time 60 -i \
  -X POST http://claude-bridge:9223/v1/messages \
  -H "Content-Type: application/json" \
  -H "anthropic-version: 2023-06-01" \
  -H "x-api-key: ollama" \
  -d "$PAYLOAD" 2>&1 | head -80

hr "9d. /v1/messages POST → claude-bridge.local:9223 (BYPASS, NO mitm audit)"
echo "Same sidecar logic — only the audit + policy layer is dropped."
echo "Useful for diffing audited vs bypass to isolate mitm-side regressions."
curl -sS --max-time 60 -i \
  -X POST http://claude-bridge.local:9223/v1/messages \
  -H "Content-Type: application/json" \
  -H "anthropic-version: 2023-06-01" \
  -H "x-api-key: ollama" \
  -d "$PAYLOAD" 2>&1 | head -80

hr "10a. claude --print 'ping' — ACTIVE env (local mode if switched)"
echo "Effective env passed to claude :"
env | grep -E "^(ANTHROPIC_|CLAUDE_CONFIG)" | sort | sed 's/^/    /' \
  || echo "    (no local-mode vars set — you're probably in cloud already)"
echo "Run :"
timeout --kill-after=5 60 claude --print 'ping' 2>&1
rc_local=$?
echo "→ exit code: $rc_local  (124=timeout, 137=SIGKILL, 0=success)"

hr "10b. claude --print 'ping' — CLOUD CONTROL (local vars stripped)"
echo "Control run with the local-mode vars unset, so claude falls back to"
echo "Anthropic cloud via ~/.claude OAuth creds. If THIS hangs too, the bug"
echo "is in the claude binary itself, not in the local-mode wiring."
echo "Run :"
timeout --kill-after=5 60 \
  env -u ANTHROPIC_BASE_URL \
      -u ANTHROPIC_AUTH_TOKEN \
      -u CLAUDE_CONFIG_DIR \
      -u ANTHROPIC_MODEL \
      -u ANTHROPIC_SMALL_FAST_MODEL \
  claude --print 'ping' 2>&1
rc_cloud=$?
echo "→ exit code: $rc_cloud  (124=timeout, 137=SIGKILL, 0=success)"

echo
echo "Compare the two responses above by eye :"
echo "  - same prompt 'ping' sent to local Ollama (10a) and Anthropic cloud (10b)"
echo "  - both exit 0 with text → both reachable ; compare content quality"
echo "  - one hangs (124/137) and the other doesn't → bug is on the failing side"

hr "11. Recent mitmproxy passive log lines for ollama.internal (success path)"
grep ollama /var/log/mitmproxy-passive.log 2>/dev/null | tail -10 \
  || echo "(no ollama entries in passive log)"

hr "11b. Recent mitmproxy passive log lines for claude-bridge (audited sidecar)"
grep claude-bridge /var/log/mitmproxy-passive.log 2>/dev/null | tail -10 \
  || echo "(no claude-bridge entries — sidecar not yet hit, or you're in cloud / local mode)"
echo "NOTE : POSTs to claude-bridge.local:9223 (bypass) should NEVER appear here ;"
echo "       if they do, NO_PROXY isn't honoring the .local match — investigate."

hr "12. Recent mitmproxy blocks log lines for ollama + claude-bridge (denied paths)"
grep -iE "ollama|claude-bridge" /var/log/mitmproxy-blocks.log 2>/dev/null | tail -10 \
  || echo "(no matching entries in blocks log)"

hr "13. iptables ACCEPT rules touching :11434 + :9223 (Ollama + sidecar host ports)"
sudo -n iptables -L OUTPUT -n -v 2>/dev/null | grep -E "11434|9223|ACCEPT" | head -15 \
  || echo "(no sudo, or no matching rules — non-fatal)"

hr "14. Compiled policy entry for ollama.internal"
sudo -n cat /var/run/devcontainer-firewall/policy.compiled.yaml 2>/dev/null \
  | awk '/^ollama\.internal:/,/^[a-z]/' | head -40 \
  || echo "(no sudo to read compiled policy — non-fatal, compare source: .devcontainer/firewall/policy.d/ollama.internal.yaml)"

hr "14b. Compiled policy entry for claude-bridge"
sudo -n cat /var/run/devcontainer-firewall/policy.compiled.yaml 2>/dev/null \
  | awk '/^claude-bridge:/,/^[a-z]/' | head -40 \
  || echo "(no sudo to read compiled policy — non-fatal, compare source: .devcontainer/firewall/policy.d/claude-bridge.yaml)"
echo "EXPECTED : /v1/messages block identical to api.anthropic.com (1:1 parity)."
echo "See .devcontainer/knowledge/INDEX.md 'Policy parity across Claude Code targets'."

hr "14c. Sidecar HTTP probe (in-container — mimics healthcheck.sh TCP probe)"
echo "The sidecar healthcheck is a TCP probe ; uvicorn doesn't expose /health."
echo "Best in-container signal : a quick POST returns a structured response."
curl -sS --max-time 5 -o /dev/null -w "HTTP %{http_code}  total=%{time_total}s  connect=%{time_connect}s\n" \
  -X POST http://claude-bridge:9223/v1/messages \
  -H "Content-Type: application/json" \
  -H "anthropic-version: 2023-06-01" \
  -H "x-api-key: ollama" \
  -d '{"model":"claude-opus-4-7","max_tokens":1,"messages":[{"role":"user","content":"."}]}' \
  2>&1 || echo "(probe failed — sidecar down or hung)"
echo "For full sidecar logs + healthcheck status, run FROM HOST :"
echo "  bash .devcontainer/host-helpers/claude-bridge logs"
echo "  bash .devcontainer/host-helpers/claude-bridge status"
echo "(both rely on the host's docker daemon, which isn't reachable in-container)"

hr "15. Summary"
echo "Log saved: $LOG"
echo "Share this file (it's in the bind-mounted .devcontainer/logs/, visible from host too)."
