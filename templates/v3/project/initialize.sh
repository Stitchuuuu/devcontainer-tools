#!/usr/bin/env bash
# The initializeCommand of a project that keeps the bash entry point.
#
# devcontainer.json says `bash .devcontainer/initialize.sh`; this file hands the
# step to the published CLI so that line never has to change. The host-side logic
# is the CLI's `devc initialize`.
#
# ══ WHY THIS FILE IS MORE THAN A ONE-LINER ═════════════════════════════════════
#
# It runs under `/bin/sh -c`, WITH NO PROFILE SOURCED. When VS Code is launched
# from a desktop icon it inherits the session manager's PATH, not your shell's.
# So nvm/fnm/asdf/volta are INVISIBLE here: `nvm use 24 --default` fixes your
# terminal and changes nothing about this step. Typing `node -v` in a terminal
# measures a different environment, which is why this script reports the
# interpreter IT resolved, and logs it.
#
# Measured failure this prevents: npm 6's npx cannot run a scoped package with a
# subcommand. It drops the package spec, treats the subcommand as a package NAME,
# and installs whatever is published under it. `initialize` is a real package on
# the public registry, so a legacy host silently downloaded and RAN a stranger's
# code, then sat at `? Package name ()`. The `--package=` form below plus the npm
# floor make that unreachable.
#
# ══ OVERRIDES, for an install this does not guess ══════════════════════════════
#   DEVC_NODE=/path/to/node     force one interpreter, skips all discovery
#   DEVC_NODE_SEARCH=dir:dir    replace the search list (colon-separated, globs ok)
#   DEVC_DEBUG=1                verbose: every candidate considered and why
#   DEVC_NODE_MIN=18            override the major floor (default 18)
#
# Undo: the file is tracked — `git checkout -- .devcontainer/initialize.sh`.
set -eu
cd "$(dirname "$0")/.."

DEVC_LOG=".devcontainer/tmp/logs/initialize-host-$(date +%Y%m%d-%H%M%S).log"
mkdir -p "$(dirname "$DEVC_LOG")" 2>/dev/null || DEVC_LOG=/dev/null
MIN="${DEVC_NODE_MIN:-18}"
DEBUG="${DEVC_DEBUG:-0}"

say()  { echo "$*"; echo "$*" >> "$DEVC_LOG" 2>/dev/null || true; }
dbg()  { [ "$DEBUG" = 1 ] && say "           $*" || echo "           $*" >> "$DEVC_LOG" 2>/dev/null || true; }
# fail <headline> — the loud stop. Says what will not work, how to fix it, and
# then OFFERS a degraded continue, because "I just need to get in and look" is a
# legitimate need and silently aborting the window open does not serve it.
#
# Enter (the empty answer) ABORTS: the safe default is the one you get by
# reflex. Only the literal word `skip` continues, and it exits 0 so VS Code
# proceeds — with the host-side setup simply not done.
#
# No TTY (VS Code does not always give one) means no question: abort, and point
# at the log.
fail() {
  say ""
  say "  ✗ initialize.sh: $1"
  say ""
  say "  WHAT THIS MEANS"
  say "    \`devc initialize\` will NOT run, so this devcontainer starts WITHOUT its"
  say "    host-side setup. Concretely, these will be missing or wrong:"
  say "      · the host-OS marker (tmp/logs/host-os) — the screenshot/CDP tooling"
  say "        then guesses your OS instead of reading it"
  say "      · .env seeding and the EXT_PATCHES_TOKEN projection — so no Allow"
  say "        button in the permission banner, and notification targeting is guessed"
  say "      · the configured/ markers — Claude mode falls back to \`dev\` silently"
  say "      · the preflight that catches a bad image pin before a 2-3 GB pull"
  say ""
  say "  HOW TO FIX, cheapest first"
  say "    1. launch VS Code from a terminal (\`code .\`) so it inherits your PATH"
  say "    2. install Node ${MIN}+ system-wide (\`brew install node\`, or your distro's)"
  say "    3. point this script straight at one: DEVC_NODE=/path/to/node"
  say "    Re-run with DEVC_DEBUG=1 to see every place that was searched."
  say ""
  say "  Full host report: $DEVC_LOG"
  say "  Give that file to Claude if you want it diagnosed."
  say ""

  # Decide BEFORE printing a prompt we may not be able to answer. `-r /dev/tty`
  # is not enough: the device node can be readable while opening it fails with
  # ENXIO (no controlling terminal), which is exactly the case under VS Code.
  # So actually try to open it.
  if [ -t 0 ]; then TTY=stdin
  elif { : < /dev/tty; } 2>/dev/null; then TTY=dev
  else TTY=none; fi

  if [ "$TTY" = none ]; then
    say "  No terminal is attached, so there is nothing to ask: ABORTING the build."
    say "  To continue degraded in a non-interactive context, set DEVC_ALLOW_DEGRADED=1."
    [ "${DEVC_ALLOW_DEGRADED:-0}" = 1 ] && { say "  → DEVC_ALLOW_DEGRADED=1: continuing DEGRADED."; exit 0; }
    exit 1
  fi

  say "  Press Enter to ABORT the build (recommended)."
  say "  Or type:  skip   then Enter, to continue anyway with the setup NOT done."
  printf '  > '
  ANS=''
  if [ "$TTY" = stdin ]; then read -r ANS || ANS=''
  else read -r ANS < /dev/tty || ANS=''; fi
  case "$ANS" in
    skip|SKIP|Skip)
      say "  → continuing DEGRADED at your request. The container will start, but"
      say "    everything listed above is not set up. Re-open once Node is fixed."
      exit 0 ;;
    *)
      say "  → aborted."
      exit 1 ;;
  esac
}

# ── 1. What kind of host is this, really ──────────────────────────────────────
UNAME="$(uname -s 2>/dev/null || echo unknown)"
KERNEL="$(uname -r 2>/dev/null || echo unknown)"
ARCH="$(uname -m 2>/dev/null || echo unknown)"
case "$UNAME" in
  Darwin)  HOST_KIND="macOS $(sw_vers -productVersion 2>/dev/null || echo '?')" ;;
  Linux)
    if [ -n "${WSL_DISTRO_NAME:-}" ] || [ -e /proc/sys/fs/binfmt_misc/WSLInterop ] \
       || case "$KERNEL" in *microsoft*|*Microsoft*|*WSL*) true ;; *) false ;; esac
    then HOST_KIND="WSL${WSL_DISTRO_NAME:+ ($WSL_DISTRO_NAME)} on Windows"
    elif [ -f /.dockerenv ]; then HOST_KIND="Linux (inside a container)"
    else HOST_KIND="Linux$( [ -r /etc/os-release ] && . /etc/os-release 2>/dev/null && printf ' %s' "${PRETTY_NAME:-}" )"
    fi ;;
  MINGW*|MSYS*|CYGWIN*) HOST_KIND="Windows ($UNAME shell) — expect PATH translation surprises" ;;
  *) HOST_KIND="$UNAME" ;;
esac

say "initialize.sh: host $HOST_KIND · $ARCH · kernel $KERNEL"
dbg "shell=$( [ -n "${BASH_VERSION:-}" ] && echo "bash $BASH_VERSION" || echo "${0##*/}" ) PATH=$PATH"
dbg "launched-by=${TERM_PROGRAM:-unknown} tty=$( [ -t 1 ] && echo yes || echo no )"

# ── 2. Find a usable Node, wherever this host keeps it ────────────────────────
# Deliberately NOT sourcing nvm.sh/fnm env: that pulls a shell library into a
# script VS Code runs on every window open, and its side effects are not ours.
# Directory layouts are a stable enough contract, and cover WSL identically
# because WSL is Linux.
vkey() { IFS=. read -r a b c <<EOF
${1#v}
EOF
  a="${a%%[!0-9]*}"; b="${b%%[!0-9]*}"; c="${c%%[!0-9]*}"
  echo $(( ${a:-0} * 1000000 + ${b:-0} * 1000 + ${c:-0} )); }

BEST=''; BEST_KEY=0; BEST_VER=''; SEEN=0
consider() {
  [ -n "${1:-}" ] || return 0
  [ -x "$1" ] || return 0
  v="$("$1" --version 2>/dev/null)" || { dbg "skip $1 (does not run)"; return 0; }
  case "$v" in v[0-9]*) ;; *) dbg "skip $1 (odd version '$v')"; return 0 ;; esac
  SEEN=$((SEEN+1)); k="$(vkey "$v")"
  if [ "$k" -lt $(( MIN * 1000000 )) ]; then dbg "skip $1 → $v (below the $MIN floor)"; return 0; fi
  if [ "$k" -le "$BEST_KEY" ]; then dbg "keep $1 → $v (not newer than $BEST_VER)"; return 0; fi
  dbg "take $1 → $v"; BEST="$1"; BEST_KEY="$k"; BEST_VER="$v"
}

if [ -n "${DEVC_NODE:-}" ]; then
  dbg "DEVC_NODE override"
  consider "$DEVC_NODE"
  [ -n "$BEST" ] || fail "DEVC_NODE=$DEVC_NODE is not an executable Node >= $MIN"
else
  consider "$(command -v node 2>/dev/null)"        # PATH first, so it wins ties
  if [ -n "${DEVC_NODE_SEARCH:-}" ]; then
    OLDIFS=$IFS; IFS=:
    for d in $DEVC_NODE_SEARCH; do IFS=$OLDIFS; for x in $d; do consider "$x/node"; done; IFS=:; done
    IFS=$OLDIFS
  else
    for d in \
      "${NVM_DIR:-$HOME/.nvm}"/versions/node/*/bin \
      "$HOME"/.fnm/node-versions/*/installation/bin \
      "$HOME"/.local/share/fnm/node-versions/*/installation/bin \
      "$HOME/Library/Application Support/fnm"/node-versions/*/installation/bin \
      "${ASDF_DATA_DIR:-$HOME/.asdf}"/installs/nodejs/*/bin \
      "${VOLTA_HOME:-$HOME/.volta}"/tools/image/node/*/bin \
      "$HOME"/.nodenv/versions/*/bin \
      "${N_PREFIX:-/usr/local}"/n/versions/node/*/bin \
      /opt/homebrew/bin /opt/homebrew/opt/node@*/bin \
      /usr/local/bin /usr/local/opt/node@*/bin \
      /opt/local/bin /snap/bin /usr/bin /bin
    do consider "$d/node"; done
  fi
fi

[ -n "$BEST" ] || fail \
"no Node >= $MIN anywhere this step can see (considered $SEEN interpreter(s)).
  This runs under /bin/sh -c with no profile sourced, so version managers are
  invisible unless their install directory is scanned — which just happened, and
  found nothing suitable. Three ways out, cheapest first:
    1. launch VS Code from a terminal (\`code .\`) where the right node is on
       PATH, so it inherits your environment;
    2. install Node $MIN+ system-wide (\`brew install node\`, or your distro's);
    3. point this script straight at one: DEVC_NODE=/path/to/node in your env.
  Re-run with DEVC_DEBUG=1 to see every place that was searched."

NODE_DIR="$(cd "$(dirname "$BEST")" && pwd -P)"
case ":$PATH:" in *":$NODE_DIR:"*) ;; *) PATH="$NODE_DIR:$PATH"; export PATH ;; esac

command -v npx >/dev/null 2>&1 || fail "found node $BEST_VER at $BEST but no npx beside it — that install is incomplete"
NPM_VER="$(npm --version 2>/dev/null || echo unknown)"; NPM_MAJOR="${NPM_VER%%.*}"
case "$NPM_MAJOR" in ''|*[!0-9]*) NPM_MAJOR=0 ;; esac

say "initialize.sh: node $BEST_VER ($BEST) · npm $NPM_VER · npx $(command -v npx)"
dbg "considered $SEEN interpreter(s); PATH now starts with $NODE_DIR"
dbg "log: $DEVC_LOG"

[ "$NPM_MAJOR" -ge 7 ] 2>/dev/null || fail \
"npm $NPM_VER is the legacy generation, whose npx cannot run a scoped package
  with a subcommand — it would install and execute an unrelated package named
  after the subcommand instead of the CLI. Node $MIN+ ships npm 9+; node 24
  ships npm 11. Refusing to continue."

# `--package=` names what to install and leaves `devc` as the binary to run, so a
# subcommand can never be mistaken for a package name. `@0.x` lets npx prefer the
# project's own devDependency copy, which keeps this offline-capable.
exec npx --yes --package=@meitogi/devcontainer-cli@0.x devc initialize "$@"
