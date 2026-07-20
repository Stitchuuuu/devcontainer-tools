#!/usr/bin/env bash
# @name gh-auth-check
# @phase post-start
# @required false
# @description GitHub CLI authentication check — user runs `gh auth login` manually in a terminal.

set -eE

if gh auth status &>/dev/null; then
  gh auth setup-git 2>/dev/null
  echo "✓ Git configured with GitHub CLI credentials."
else
  echo ""
  echo "⚠️  ========================================"
  echo "⚠️  GitHub CLI is NOT authenticated!"
  echo "⚠️  Git push/pull will not work."
  echo "⚠️  Open a terminal and run: gh auth login"
  echo "⚠️  ========================================"
  echo ""
fi
