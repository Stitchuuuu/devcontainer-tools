# -----------------------------------------------
# Project DevContainer — slim layer.
# All heavy lifting lives in `Dockerfile.base`, built once per
# CLAUDE_CODE_VERSION by `.devcontainer/initialize.sh` and tagged
# `claude-devcontainer-base:${CLAUDE_CODE_VERSION}-${DC_PROJECT}`.
#
# To bump Claude: edit CLAUDE_CODE_VERSION in `.devcontainer/.env` then
# "Rebuild Container" in VS Code. The initializeCommand will detect the
# missing base tag and rebuild it before compose builds this layer.
# -----------------------------------------------
ARG CLAUDE_CODE_VERSION=2.1.145
ARG DC_PROJECT={{PROJECT_ID}}

# -----------------------------------------------
# Stage 1 — firewall bake (throwaway)
# -----------------------------------------------
# Why a separate stage rather than one COPY + RUN in the final image :
# firewall/ carries domains.local.txt and policy.local.d/, which are gitignored
# and writable by anything running in the container, an npm postinstall
# included. A `RUN rm` after the COPY would not help — the file still sits in
# the COPY layer, so `docker save` and any registry push still ship it. Layers
# built in a stage that nothing copies from never enter the final manifest, so
# the .local files are discarded rather than merely hidden.
#
# The bake also freezes the compiled ruleset into effective/, which boot
# installs verbatim instead of recompiling.
FROM claude-devcontainer-base:${CLAUDE_CODE_VERSION}-${DC_PROJECT} AS fw-bake

# 0 = hardened (default) : domains.local.txt and policy.local.d/ are NOT baked.
# 1 = convenience : they are. Set per developer through .env, never as a team
# default. Either way the bake logs which it did and how many hosts it costs.
ARG FIREWALL_ALLOW_LOCAL_AT_REBUILD=0

USER root
COPY firewall/ /tmp/fw-src/
RUN FIREWALL_ALLOW_LOCAL_AT_REBUILD="${FIREWALL_ALLOW_LOCAL_AT_REBUILD}" \
    /usr/local/bin/firewall-docker-setup.sh --src /tmp/fw-src --dest /out

# -----------------------------------------------
# Final image
# -----------------------------------------------
FROM claude-devcontainer-base:${CLAUDE_CODE_VERSION}-${DC_PROJECT}

# Project-specific layer (empty by default — add project deps here).
# Add `RUN apt-get install -y …` here for project apt deps that should NOT
# live in the shared base image.

USER root

# /out already merges the base image's firewall content with the project
# overlay, so this single COPY is the whole configuration. Ownership and modes
# were set root:root / u=rwX,go=rX in the bake stage and COPY --from preserves
# them. The assertion catches a bake that silently produced nothing.
COPY --from=fw-bake /out/ /etc/devcontainer-firewall/
RUN test -s /etc/devcontainer-firewall/baked-at \
 && test -s /etc/devcontainer-firewall/effective/sources.sha256

USER node
