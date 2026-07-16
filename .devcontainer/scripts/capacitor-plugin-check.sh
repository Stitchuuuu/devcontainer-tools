#!/usr/bin/env bash
# capacitor-plugin-check.sh — Capacitor Android plugin delivery gate.
# Compiles all .kt / .java under android/src/main/ against the baked
# android.jar matrix (default APIs: 23 24 26 28 31 33 34 35) plus the
# Capacitor + AndroidX + kotlinx deps baked in /opt/cap-deps/, plus any
# on-demand libs cached in /workspace/.devcontainer/cache/android-jars/maven/.
#
# Invoke before considering a change ready to deliver (commit / PR /
# "task done"). Exits non-zero if any API fails to compile.
#
# Usage :
#   bash .devcontainer/scripts/capacitor-plugin-check.sh
#   bash .devcontainer/scripts/capacitor-plugin-check.sh path/to/src
set -e
cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
ROOT="${1:-android/src/main}"
if [ ! -d "$ROOT" ]; then
  echo "ERROR: source dir not found: $ROOT"
  echo "usage: $0 [source-dir]  (default: android/src/main)"
  exit 1
fi
files=$(find "$ROOT" -type f \( -name "*.kt" -o -name "*.java" \))
if [ -z "$files" ]; then
  echo "No .kt / .java files found under $ROOT"
  exit 0
fi
count=$(echo "$files" | wc -l | tr -d ' ')
echo "Compile-matrix check on $count files under $ROOT:"
echo "$files" | sed 's/^/  /'
echo
kchk-matrix $files
