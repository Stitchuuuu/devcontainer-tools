#!/usr/bin/env bash
# @name firewall-local-banner
# @phase post-start
# @required false
# @description One-line info banner if local overrides are active. Committed policy is augmented locally — useful to flag for visibility, no enforcement implication.

set -eE

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
