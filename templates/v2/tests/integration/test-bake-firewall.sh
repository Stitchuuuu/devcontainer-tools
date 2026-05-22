#!/usr/bin/env bash
# integration/test-bake-firewall.sh — runtime validation of session 1.
#
# Requires : running inside a rebuilt devcontainer (post-bake). Each test
# auto-skips if the bake hasn't taken effect yet (bind mount detected) or
# if we're not inside any container.

source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"

# Heuristic : we're post-bake if /etc/devcontainer-firewall/ is NOT a mount
# point (pre-bake it's a `:ro` bind mount from docker-compose). The previous
# heuristic — `touch /etc/...` → EROFS — gave false positives because the
# `:ro` mount also returns EROFS on direct writes (that's the cosmetic flaw
# session 1 fixes).
_post_bake() {
  in_container || return 1
  [ -d /etc/devcontainer-firewall ] || return 1
  # `findmnt` exits 0 if the path is a mount point. Post-bake the dir is
  # part of the image's filesystem, not a mount.
  if command -v findmnt >/dev/null 2>&1; then
    findmnt -n /etc/devcontainer-firewall >/dev/null 2>&1 && return 1
  else
    # Fallback : awk on /proc/self/mountinfo.
    awk '$5 == "/etc/devcontainer-firewall" {found=1} END {exit !found}' /proc/self/mountinfo 2>/dev/null && return 1
  fi
  return 0
}

test_reads_work() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  assert_file_exists /etc/devcontainer-firewall/domains.txt          "domains.txt readable"
  assert_file_exists /etc/devcontainer-firewall/default-mode         "default-mode readable"
  assert_file_exists /etc/devcontainer-firewall/direct-tcp-allow.txt "direct-tcp-allow.txt readable"
  assert_dir_exists  /etc/devcontainer-firewall/policy.local.d       "policy.local.d/ readable"
}

test_writes_to_etc_blocked() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  assert_false touch /etc/devcontainer-firewall/test-attack-$$ -- "touch /etc/ blocked (EROFS)"
  rm -f /etc/devcontainer-firewall/test-attack-$$ 2>/dev/null
  local probe="evil-$$-$(date +%s)"
  assert_false bash -c "echo $probe >> /etc/devcontainer-firewall/domains.local.txt" -- \
    "echo >> /etc/domains.local.txt blocked"
}

test_workspace_decoupled_from_etc() {
  _post_bake || { skip_test "not in post-bake container"; return; }
  [ -w /workspace/.devcontainer/firewall/domains.local.txt ] || {
    skip_test "workspace domains.local.txt not writable" ; return ; }
  local poc="decouple-poc-$$-$(date +%s)"
  echo "$poc" >> /workspace/.devcontainer/firewall/domains.local.txt
  sync
  assert_false grep -qF "$poc" /etc/devcontainer-firewall/domains.local.txt -- \
    "workspace write does NOT leak to /etc (decoupled)"
  sed -i "/$poc/d" /workspace/.devcontainer/firewall/domains.local.txt 2>/dev/null
}

test_firewall_active() {
  # Behavioural proof : iptables DROP rules are observable via curl, no need
  # to read iptables directly (node doesn't have CAP_NET_ADMIN or arbitrary
  # sudo — only init-firewall.sh / test-firewall.sh are sudoers-allowed).
  # Mode-aware : `off` is the kill-switch — google.com is *expected* to be
  # reachable. Per-mode behavior is asserted in test-firewall-modes.sh.
  _post_bake || { skip_test "not in post-bake container"; return; }
  local mode; mode=$(cat /etc/devcontainer-firewall/default-mode 2>/dev/null | tr -d '[:space:]')
  case "$mode" in
    off)
      if curl -sSf --max-time 3 https://google.com >/dev/null 2>&1; then
        _ok "google.com reachable in off mode (kill-switch active)"
      else
        _nok "google.com blocked in off mode (kill-switch broken?)"
      fi
      ;;
    strict|basic)
      if curl -sSf --max-time 3 https://google.com >/dev/null 2>&1; then
        _nok "google.com reachable in $mode (firewall inactive?)"
      else
        _ok "google.com blocked in $mode (firewall DROP active)"
      fi
      ;;
    *) skip_test "unknown mode '$mode'" ; return ;;
  esac
  # api.anthropic.com : reachable in all modes (always allowlisted). Response
  # code differs : 401/403 via mitm in strict, anything 2xx-4xx direct in basic/off.
  local code
  code=$(curl -sk --max-time 5 https://api.anthropic.com/v1/models -o /dev/null -w "%{http_code}" 2>/dev/null)
  if [ -n "$code" ] && [ "$code" != "000" ]; then
    _ok "api.anthropic.com reachable in $mode (HTTP $code)"
  else
    _nok "api.anthropic.com unreachable in $mode (curl code '$code')"
  fi
}

test_vector12_inert() {
  # Vector #12 : flipping firewall/default-mode in workspace must NOT change
  # runtime. Use a sentinel string (not a valid mode) so we can assert the
  # sentinel never appears in /etc, regardless of what the baked mode
  # currently is (strict / basic / off — all valid baselines).
  _post_bake || { skip_test "not in post-bake container"; return; }
  [ -w /workspace/.devcontainer/firewall/default-mode ] || {
    skip_test "workspace default-mode not writable" ; return ; }
  local orig sentinel="sentinel-vector-12-$$-$(date +%s)"
  orig=$(cat /workspace/.devcontainer/firewall/default-mode)
  echo "$sentinel" > /workspace/.devcontainer/firewall/default-mode
  sync
  local runtime; runtime=$(cat /etc/devcontainer-firewall/default-mode | tr -d '[:space:]')
  assert_ne "$sentinel" "$runtime" "vector #12 inert : workspace sentinel did NOT leak to /etc"
  echo "$orig" > /workspace/.devcontainer/firewall/default-mode
}

test_vector13_inert() {
  # Vector #13 : adding evil:port to workspace direct-tcp-allow must not propagate.
  _post_bake || { skip_test "not in post-bake container"; return; }
  [ -w /workspace/.devcontainer/firewall/direct-tcp-allow.txt ] || {
    skip_test "workspace direct-tcp-allow not writable" ; return ; }
  echo "evil.com:443" >> /workspace/.devcontainer/firewall/direct-tcp-allow.txt
  sync
  assert_false grep -qF "evil.com:443" /etc/devcontainer-firewall/direct-tcp-allow.txt -- \
    "vector #13 inert : workspace direct-tcp-allow not visible at runtime"
  sed -i '/evil\.com:443/d' /workspace/.devcontainer/firewall/direct-tcp-allow.txt 2>/dev/null
}

run_tests
