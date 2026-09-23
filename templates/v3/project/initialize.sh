#!/usr/bin/env bash
# The initializeCommand of a project that keeps the bash entry point.
#
# devcontainer.json says `bash .devcontainer/initialize.sh`, as every tree made
# by install.sh does. This file hands the step to the published CLI, so that
# line never has to change. The host-side logic is the CLI's `devc initialize`
# (its port of the 663-line v2 script); nothing else happens here, on purpose:
# a fallback to the old script would hide the one failure this shim can meet —
# a Dockerfile still FROMing the local claude-devcontainer-base that only the
# old script built. Switch the Dockerfile first, then install this.
#
# The range spec (`@0.x`) is what makes npx run the project's own copy: with the
# CLI as a root devDependency (`npm i -D @meitogi/devcontainer-cli`, which
# `devc init` does for a scaffolded project) no registry is contacted and the
# container starts offline. Without it, npx resolves the range online at every
# start, and this step fails without network.
#
# Needs Node 18+ on the host, the same requirement as `devc init`. Undo: the
# file is tracked — `git checkout -- .devcontainer/initialize.sh`.
set -eu
cd "$(dirname "$0")/.."
if ! command -v npx >/dev/null 2>&1; then
  echo "initialize.sh: npx not found — install Node.js 18+ on the host; this step runs before any container exists" >&2
  exit 1
fi
exec npx --yes @meitogi/devcontainer-cli@0.x initialize "$@"
