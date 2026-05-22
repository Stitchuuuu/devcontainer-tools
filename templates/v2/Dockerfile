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

# Project-specific firewall data — baked into the image, no runtime bind mount.
# Recursive COPY embarks the whole firewall/ tree (domains, policy.d/,
# policy.local.d/, default-mode, direct-tcp-allow.txt, addons, dnsmasq.conf,
# tests, …). Base-layer COPYs into /etc/devcontainer-firewall/ from
# Dockerfile.base are overlaid by this COPY where paths overlap — projects
# can override addons/, dnsmasq.conf, tests/ that way. firewall-docker-setup.sh
# (baked in the base image) finalizes perms + chown.
COPY firewall/ /etc/devcontainer-firewall/
RUN /usr/local/bin/firewall-docker-setup.sh

USER node
