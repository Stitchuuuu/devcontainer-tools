#!/usr/bin/env bash
# @name watch-log-cleanup
# @phase post-start
# @required false
# @description Watch-log cleanup — drop pending/* > 60 min stale (skill /watch-log, C).

set -eE

CLEANUP=/workspace/.devcontainer/host-helpers/watch-log-cleanup
[ -x "$CLEANUP" ] && "$CLEANUP"
