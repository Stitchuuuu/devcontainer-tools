#!/usr/bin/env bash
# agent-browser-kit.sh — installs / checks / removes the agent browser's
# LaunchAgent on the Mac (devcontainer-sandbox 1.13.0+, T1 of ctr-native-web).
# HOST ONLY : run it in the Mac's Terminal, from the workspace (wtf does that).
#
#   wtf agent-browser install     extract the kit from the image, then install
#   wtf agent-browser status      launchd state, heartbeat, kit hash
#   wtf agent-browser uninstall   remove the LaunchAgent and its folder
#   wtf agent-browser logs        follow the supervisor, proxy and Chromium logs
#
# The kit is extracted from the devcontainer image (`agent-browser host-kit`),
# so what launchd runs never came out of the container's writable tree. It
# lands in ~/Library/Application Support/agent-browser/<DC_PROJECT>/kit.
# Needs : the container rebuilt on an image that ships agent-browser (wtf base
# dev, then Rebuild Container), node 22+ and Chromium on the Mac
# (brew install --cask chromium), AGENT_BROWSER_ORIGINS and host:9223 set in
# .devcontainer (already done for this workspace).
set -euo pipefail

action=${1:-}
case "$action" in
  install|status|uninstall|logs) ;;
  *) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac

[ "$(uname -s)" = Darwin ] || { echo "agent-browser-kit: host only — run it in the Mac's Terminal" >&2; exit 2; }

root=$(cd "$(dirname "$0")/../.." && pwd)
project=$(sed -n 's/^DC_PROJECT=//p' "$root/.devcontainer/.env" | tail -1)
[ -n "$project" ] || { echo "agent-browser-kit: no DC_PROJECT in .devcontainer/.env" >&2; exit 2; }
kit="$HOME/Library/Application Support/agent-browser/$project/kit"

if [ "$action" = install ]; then
  mkdir -p "$kit"
  echo "→ extracting the kit from the devcontainer image into $kit"
  docker compose -f "$root/.devcontainer/docker-compose.yml" run --rm --no-deps -T app agent-browser host-kit \
    | tar -x -C "$kit" \
    || { echo "agent-browser-kit: extraction failed — does the image ship agent-browser ? (wtf base dev, then Rebuild Container)" >&2; exit 1; }
fi

[ -f "$kit/host-agent.sh" ] || { echo "agent-browser-kit: no kit at $kit — run: wtf agent-browser install" >&2; exit 1; }

# Resolve Chromium here, in the user's shell, and hand the path to install
# (the environment wins over .env) : the kit's supervisor searches again under
# launchd and misses a Playwright Chromium this shell finds. The robrowser
# agent does the same (ROBROWSER_CHROMIUM baked at install).
if [ "$action" = install ] && [ -z "${AGENT_BROWSER_CHROMIUM:-}" ]; then
  for app in /Applications/Chromium.app "$HOME/Applications/Chromium.app" \
    "$HOME"/Library/Caches/ms-playwright/chromium-*/chrome-mac*/Chromium.app; do
    if [ -x "$app/Contents/MacOS/Chromium" ]; then
      export AGENT_BROWSER_CHROMIUM="$app/Contents/MacOS/Chromium"
      echo "→ Chromium : ${AGENT_BROWSER_CHROMIUM/#$HOME/~}"
      break
    fi
  done
fi
exec bash "$kit/host-agent.sh" "$action"
