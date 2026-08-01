#!/usr/bin/env bash
# @name firewall-bake-warn
# @phase post-start
# @required false
# @description Red banner when the image carries no baked firewall, or when the baked ruleset no longer matches its sources. Both mean the frozen set was bypassed and boot fell back to compiling whatever /etc happened to hold — which is exactly the state the bake exists to make impossible.

set -eE

# Overridable so firewall/tests/ can render both banners against a sandbox —
# a red banner nobody has ever seen fire is a banner that fires wrong.
FW="${FIREWALL_CONFIG_DIR:-/etc/devcontainer-firewall}"
EFFECTIVE="$FW/effective"
DIGEST_LIB="${FW_DIGEST_LIB:-/usr/local/bin/firewall-digest.sh}"

banner() {
  printf '\033[1;31m'
  printf '%s\n' "$@"
  printf '\033[0m\n'
}

# Case 1 — the image was never baked. A project Dockerfile built before the
# bake existed, or one that dropped the firewall block on a rebase.
if [ ! -s "$FW/baked-at" ] || [ ! -s "$EFFECTIVE/sources.sha256" ]; then
  banner \
    '╔════════════════════════════════════════════════════════════════╗' \
    '║  ⚠  FIREWALL NOT BAKED — this image has no frozen ruleset      ║' \
    '║                                                                ║' \
    '║  Boot compiled whatever /etc/devcontainer-firewall held, so    ║' \
    '║  domains.local.txt is live without anyone having approved it.  ║' \
    '║                                                                ║' \
    '║  Your .devcontainer/Dockerfile is missing the bake stage :     ║' \
    '║     COPY firewall/ /tmp/fw-src/                                ║' \
    '║     RUN firewall-docker-setup.sh --src /tmp/fw-src --dest /out ║' \
    '║  then Rebuild Container.                                       ║' \
    '╚════════════════════════════════════════════════════════════════╝'
  exit 0
fi

# Case 2 — baked, but the sources moved since. init-firewall.sh already logged
# the fallback and recompiled ; this surfaces it where someone will see it,
# because the recompile silently re-admits anything sitting in /etc.
if [ -r "$DIGEST_LIB" ]; then
  # shellcheck source=/dev/null
  . "$DIGEST_LIB"
  BAKED=$(cat "$EFFECTIVE/sources.sha256")
  CURRENT=$(fw_sources_digest "$FW" "$(cat "$EFFECTIVE/local-included" 2>/dev/null || echo 0)")
  if [ "$BAKED" != "$CURRENT" ]; then
    banner \
      '╔════════════════════════════════════════════════════════════════╗' \
      '║  ⚠  FIREWALL DRIFT — the baked ruleset no longer matches       ║' \
      '║     the sources in /etc/devcontainer-firewall.                 ║' \
      '║                                                                ║' \
      '║  Boot recompiled instead of using the frozen set. Rebuild the  ║' \
      '║  container to re-bake, or inspect what changed :               ║' \
      '║     reload-firewall --dry-run                                  ║' \
      '╚════════════════════════════════════════════════════════════════╝'
    printf '   baked %s → current %s\n' "${BAKED:0:12}" "${CURRENT:0:12}"
    exit 0
  fi
fi

# Silence on the happy path — the safe state does not need a banner.
