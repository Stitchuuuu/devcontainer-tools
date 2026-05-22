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
