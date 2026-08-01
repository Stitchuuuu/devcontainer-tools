#!/usr/bin/env bash
# reload-local.sh — SUPERSEDED by /usr/local/bin/reload-firewall.
#
# This file used to be the hot-reload engine, executed as root through
# `docker exec -u 0 /workspace/.devcontainer/reload-local.sh`. That was the
# problem : /workspace is writable by everything in the container, so anything
# that could edit this file — an npm postinstall, an agent — got arbitrary root
# execution the next time a human typed `wtf firewall reload`.
#
# The replacement lives in the image, root-owned, at
# /usr/local/bin/reload-firewall. It also adds what this script never had : a
# --dry-run that needs no privileges, a diff of the candidate ruleset, and an
# interactive confirmation. Source : bin/reload-firewall.
#
# Deliberately refuses instead of forwarding. A silent exec would leave callers
# believing they still run the old semantics, and they differ : reload-firewall
# never writes /etc/devcontainer-firewall, so a reload lasts only as long as
# the container and the next start returns to the baked ruleset.

cat >&2 <<'EOF'
❌ reload-local.sh has been replaced by reload-firewall.

  Preview the diff (no root needed) :
      reload-firewall --dry-run

  Apply, from a host terminal :
      wtf firewall reload
      docker exec -it -u 0 <container> /usr/local/bin/reload-firewall

Differences worth knowing :
  · the .local layer is no longer baked into the image by default, so a
    reload is how it reaches a running container ;
  · a reload is ephemeral — restarting the container returns to the baked
    ruleset. Set FIREWALL_ALLOW_LOCAL_AT_REBUILD=1 to bake it in instead.
EOF
exit 1
