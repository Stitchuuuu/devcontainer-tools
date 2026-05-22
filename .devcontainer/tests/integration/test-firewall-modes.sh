#!/usr/bin/env bash
# integration/test-firewall-modes.sh — verify the baked mode is in effect.
#
# Each `test_mode_*` skips unless the baked mode matches its expected value.
# To exercise all 3 modes, rebuild the container once per mode (off / basic
# / strict) and re-run this suite.

source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"

_post_bake() {
  in_container || return 1
  [ -d /etc/devcontainer-firewall ] || return 1
  if command -v findmnt >/dev/null 2>&1; then
    findmnt -n /etc/devcontainer-firewall >/dev/null 2>&1 && return 1
  else
    awk '$5 == "/etc/devcontainer-firewall" {found=1} END {exit !found}' /proc/self/mountinfo 2>/dev/null && return 1
  fi
  return 0
}

_mode() {
  cat /etc/devcontainer-firewall/default-mode 2>/dev/null | tr -d '[:space:]'
}

test_mode_strict_invariants() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  [ "$(_mode)" = "strict" ] || { skip_test "baked mode = $(_mode), not strict"; return; }
  # mitmproxy listening on 127.0.0.1:8080 — probe with a no-network HTTP req.
  # (`ss -ltn` requires CAP_NET_ADMIN to see other users' sockets ; the curl
  # probe is the user-mode equivalent — connection refused vs timeout tells us.)
  if curl -sk --max-time 3 http://127.0.0.1:8080 >/dev/null 2>&1; then
    _ok "mitmproxy reachable on 127.0.0.1:8080"
  else
    # Even when mitm returns an error it accepts the TCP connect. A pure
    # "connection refused" means nothing listens.
    local err
    err=$(curl -sk --max-time 3 http://127.0.0.1:8080 2>&1)
    if echo "$err" | grep -qi "connection refused"; then
      _nok "mitmproxy NOT listening on 127.0.0.1:8080 (connection refused)"
    else
      _ok "mitmproxy reachable on 127.0.0.1:8080 (non-refused response)"
    fi
  fi
  local code
  code=$(curl -sk --max-time 5 https://api.anthropic.com/v1/models -o /dev/null -w "%{http_code}" 2>/dev/null)
  case "$code" in
    401|403) _ok "api.anthropic.com reachable via mitm (HTTP $code)" ;;
    *)       _nok "api.anthropic.com unexpected HTTP $code (expected 401/403)" ;;
  esac
  assert_false curl -sSf --max-time 3 https://google.com -- "non-allowlisted google.com blocked"
}

test_mode_basic_invariants() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  [ "$(_mode)" = "basic" ] || { skip_test "baked mode = $(_mode), not basic"; return; }
  # In basic : DNS allowlist active, no mitmproxy.
  local code
  code=$(curl -s --max-time 5 https://api.anthropic.com/v1/models -o /dev/null -w "%{http_code}" 2>/dev/null)
  [ -n "$code" ] && _ok "api.anthropic.com reachable in basic (HTTP $code)" \
                 || _nok "api.anthropic.com unreachable in basic"
  assert_false curl -sSf --max-time 3 https://google.com -- "google.com blocked at DNS layer (basic)"
}

test_mode_off_invariants() {
  # Off mode is a full bypass : iptables flushed, no DNS allowlist, no mitm.
  # The behavioural proof = google.com is reachable directly (no proxy, no
  # allowlist). If google.com works in off mode, the kill-switch worked.
  _post_bake || { skip_test "not in post-bake container"; return; }
  [ "$(_mode)" = "off" ] || { skip_test "baked mode = $(_mode), not off"; return; }
  if curl -sSf --max-time 5 https://google.com >/dev/null 2>&1; then
    _ok "google.com reachable in off mode (kill-switch works)"
  else
    _nok "google.com unreachable in off mode (kill-switch broken — flush failed?)"
  fi
  # Sanity : mitmproxy must NOT be in the path (HTTPS_PROXY should be cleared
  # in off mode by firewall-mode.sh — env-file driven).
  if [ -n "${HTTPS_PROXY:-}" ] && [ "${HTTPS_PROXY}" != "" ]; then
    _nok "HTTPS_PROXY still set in off mode : $HTTPS_PROXY (firewall-mode.sh didn't clear it before rebuild?)"
  else
    _ok "HTTPS_PROXY cleared in off mode"
  fi
}

run_tests
