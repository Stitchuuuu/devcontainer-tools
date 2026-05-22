#!/usr/bin/env bash
# firewall-docker-setup.sh — runs ONCE at project-layer Docker build time
# (RUN'd from .devcontainer/Dockerfile or .devcontainer/Dockerfile.php).
#
# Finalizes perms/ownership on the project-specific firewall data COPY'd
# into /etc/devcontainer-firewall/ by the project Dockerfile :
#   - domains.txt           (per-project baseline allowlist)
#   - policy.d/             (per-project L7 policies)
#
# Also touches an empty domains.local.txt that init-firewall.sh expects
# at runtime (whether or not the project provides local overrides).
#
# Lives in the base image (/usr/local/bin/) — one source of truth, evolves
# per CLAUDE_CODE_VERSION bump without forcing project-side re-install.sh.
#
# capital-X in chmod = "x only if dir or already +x" — gives 755 on dirs
# + 644 on files in one pass, no per-path enumeration. Idempotent on
# base-layer files (already at those perms, no-op).
set -euo pipefail

FW=/etc/devcontainer-firewall

touch "$FW/domains.local.txt"
chown -R root:root "$FW"
chmod -R u=rwX,go=rX "$FW"
