# -----------------------------------------------
# Project DevContainer — slim layer.
# All heavy lifting lives in `Dockerfile.base`, built once per
# CLAUDE_CODE_VERSION by `.devcontainer/initialize.sh` and tagged
# `claude-devcontainer-base:${CLAUDE_CODE_VERSION}`.
#
# To bump Claude: edit CLAUDE_CODE_VERSION in `.devcontainer/.env` then
# "Rebuild Container" in VS Code. The initializeCommand will detect the
# missing base tag and rebuild it before compose builds this layer.
# -----------------------------------------------
ARG CLAUDE_CODE_VERSION=2.1.145
FROM claude-devcontainer-base:${CLAUDE_CODE_VERSION}

# Project-specific layer (empty by default — add project deps here).
# Add `RUN apt-get install -y …` here for project apt deps that should NOT
# live in the shared base image.

USER root

# Project-specific firewall data — rebuilds per-project ; does NOT affect
# the shared claude-devcontainer-base image. firewall-docker-setup.sh
# lives in the base image ; touches domains.local.txt + finalizes perms.
# *.example files are NOT COPY'd : they're host-side reference only
# (accessible from inside via /workspace/.devcontainer/firewall/ bind mount).
COPY firewall/domains.txt /etc/devcontainer-firewall/domains.txt
COPY firewall/policy.d/   /etc/devcontainer-firewall/policy.d/
RUN /usr/local/bin/firewall-docker-setup.sh

USER node
