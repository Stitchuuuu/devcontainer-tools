#!/usr/bin/env bash
#
# .devcontainer/host-helpers/cleanup.sh — `wtf cleanup`: reclaim disk, touching
# only what is safe to lose. Same command in the devcontainer and on the Mac;
# the Mac does more, because only it sees Docker Desktop.
#
#   cleanup.sh                  report only, nothing removed
#   cleanup.sh --apply          do it
#   cleanup.sh --all [--apply]  on the Mac, also every OTHER running devcontainer
#
# Inside the devcontainer:
#   devc-sweep for this container — abandoned test temp dirs, scratch dirs of
#   ended Claude sessions, transcripts > 30 days ARCHIVED (devc-sweep restore).
#
# On the Mac (macOS bash 3.2 compatible):
#   1. the same devc-sweep, inside this project's devcontainer — found by its
#      compose labels (project <DC_PROJECT>-claude-code, service app) ; when it
#      is not running the section says so and moves on.
#   2. Docker — docker-reclaim.sh's safe targets: build cache backing no
#      image, untagged images no container uses, idle buildx builders,
#      anonymous volumes with zero links.
#   3. with --all, devc-sweep in every other running sandbox devcontainer
#      (its image carries org.stitchu.base.version).
#
# It NEVER touches named volumes (databases, Claude configs), stopped
# containers, tagged images or live Claude sessions. Stopped containers that
# pin images are listed by docker-reclaim's report for you to decide.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APPLY=0
ALL=0
for a in "$@"; do
    case "$a" in
        --apply) APPLY=1 ;;
        --all)   ALL=1 ;;
        -h|--help) sed -n '2,28p' "$0" | sed 's/^#\{1,\} \{0,1\}//'; exit 0 ;;
        *) echo "cleanup: unknown argument: $a (--apply, --all)" >&2; exit 64 ;;
    esac
done
SWEEP_ARGS=""; [ "$APPLY" -eq 1 ] && SWEEP_ARGS="--apply"
# devc-sweep's closing hint names the command to type: here, this one.
HINT="wtf cleanup --apply$([ "$ALL" -eq 1 ] && echo ' --all')"

# ------------------------------------------------------------ inside a container
# There `docker` is the nested daemon, not Docker Desktop: only this
# container's own sweep makes sense. devc-sweep is the image's (1.12+), else
# the one in this workspace's sandbox checkout. CLEANUP_ALLOW_NESTED forces
# the Mac path, for this script's own test only.
if [ -f /.dockerenv ] && [ "${CLEANUP_ALLOW_NESTED:-0}" != 1 ]; then
    SWEEP=""
    if command -v devc-sweep >/dev/null 2>&1; then
        SWEEP="devc-sweep"
    elif [ -x "$ROOT/packages/devcontainer-sandbox/bin/devc-sweep" ]; then
        SWEEP="$ROOT/packages/devcontainer-sandbox/bin/devc-sweep"
    fi
    [ -n "$SWEEP" ] || { echo "cleanup: no devc-sweep in this container (image older than 1.12) — rebuild on 1.12+" >&2; exit 1; }
    printf '\033[1mwtf cleanup — DEVCONTAINER mode\033[0m\n'
    echo "  Cleans THIS container only: abandoned temp dirs, scratch dirs of ended"
    echo "  Claude sessions, transcripts > 30 days archived (devc-sweep)."
    echo "  More can be reclaimed from the Mac, where \`wtf cleanup\` also cleans:"
    echo "    - Docker Desktop: build cache, unused untagged images, idle builders,"
    echo "      orphan anonymous volumes (often the bulk of the disk)"
    echo "    - with --all, every other running devcontainer"
    [ "$ALL" -eq 1 ] && echo "  (--all only applies on the Mac — here it changes nothing.)"
    echo
    DEVC_SWEEP_APPLY_HINT="$HINT" exec "$SWEEP" $SWEEP_ARGS
fi

# ------------------------------------------------------------ on the Mac
docker info >/dev/null 2>&1 || { echo "cleanup: docker daemon unreachable — is Docker Desktop running?" >&2; exit 1; }

bold() { printf '\n\033[1m%s\033[0m\n' "$1"; }
gb()   { awk -v b="${1:-0}" 'BEGIN{printf "%.1f GB", b/1e9}'; }

# Bytes used on the VM disk, from a throwaway container of an image already
# present (no pull, no network) — the same probe as docker-reclaim.
vm_used() {
    local img
    for img in $(docker images --format '{{.ID}}' | head -5); do
        docker run --rm --network none --entrypoint df "$img" -k / 2>/dev/null \
            | awk 'NR==2 { printf "%.0f\n", $3*1024; ok=1 } END { exit !ok }' && return 0
    done
    return 1
}

SUM="$(mktemp)" || { echo "cleanup: mktemp failed (disk full?)" >&2; exit 1; }
trap 'rm -f "$SUM"' EXIT

# relay <tag> — show a part's output indented, and keep its machine-readable
# @@ totals line (tagged) for the closing summary instead of printing it.
relay() {
    local tag="$1" line
    while IFS= read -r line; do
        line="${line%$'\r'}"
        case "$line" in
            @@*) echo "$tag ${line#@@* }" >> "$SUM" ;;
            *)   printf '    %s\n' "$line" ;;
        esac
    done
}

# project_of <container> — its compose project without the -claude-code suffix.
project_of() {
    local p
    p=$(docker inspect -f '{{index .Config.Labels "com.docker.compose.project"}}' "$1" 2>/dev/null)
    p="${p%-claude-code}"
    echo "${p:-$1}"
}

# sweep_in <container> — devc-sweep inside it as node. The image's copy
# (1.12+) ; on an older image, this workspace's copy streamed in through
# stdin, so every project is covered before its rebuild.
sweep_in() {
    local c="$1" img ver name
    name=$(project_of "$c")
    img=$(docker inspect -f '{{.Image}}' "$c" 2>/dev/null)
    ver=$(docker image inspect -f '{{index .Config.Labels "org.stitchu.base.version"}}' "$img" 2>/dev/null)
    printf '\n\033[1m  %s\033[0m  (container %s, base %s)\n' "$name" "$c" "${ver:-?}"
    if docker exec "$c" test -x /usr/local/bin/devc-sweep 2>/dev/null; then
        docker exec -u node -e "DEVC_SWEEP_APPLY_HINT=$HINT" -e DEVC_SWEEP_SUMMARY=1 "$c" \
            devc-sweep $SWEEP_ARGS | relay "project $name"
    elif [ -f "$ROOT/packages/devcontainer-sandbox/bin/devc-sweep" ]; then
        echo "    (image older than 1.12 — using this workspace's devc-sweep)"
        docker exec -i -u node -e "DEVC_SWEEP_APPLY_HINT=$HINT" -e DEVC_SWEEP_SUMMARY=1 "$c" \
            bash -s -- $SWEEP_ARGS < "$ROOT/packages/devcontainer-sandbox/bin/devc-sweep" | relay "project $name"
    else
        echo "    skipped: image older than 1.12, no devc-sweep — rebuild it on 1.12+"
        echo "project $name skipped" >> "$SUM"
    fi
}

# This project's devcontainer: compose project <DC_PROJECT>-claude-code,
# service app (.devcontainer/docker-compose.yml). DC_PROJECT from .env.
DC_PROJECT=$(sed -n 's/^DC_PROJECT=//p' "$ROOT/.devcontainer/.env" 2>/dev/null | tail -1)
DC_PROJECT="${DC_PROJECT:-devcontainer-tools}"
SELF=$(docker ps --filter "label=com.docker.compose.project=${DC_PROJECT}-claude-code" \
                 --filter "label=com.docker.compose.service=app" --format '{{.Names}}' | head -1)

printf '\033[1mwtf cleanup — MAC mode\033[0m  %s\n' "$([ "$APPLY" -eq 1 ] && echo 'APPLY' || echo 'report only — nothing is removed')"
echo "  In each project's devcontainer — see each section for why it is safe:"
echo "    test temp dirs nobody removed, working files of ended Claude sessions,"
echo "    conversations untouched for 30 days (ARCHIVED, restorable)."
echo "  In Docker Desktop: build cache no image uses, untagged images no"
echo "    container uses, idle buildx builders, anonymous volumes nothing links."
echo "  Never: named volumes (databases, Claude configs), stopped containers,"
echo "    tagged images, running Claude sessions."
BEFORE=$(vm_used || echo "")

# --------------------------------------------------------------------------
bold "── 1. This project's devcontainer (${DC_PROJECT})"
if [ -n "$SELF" ]; then
    sweep_in "$SELF"
else
    echo "  not connected — no running container for compose project ${DC_PROJECT}-claude-code."
    echo "  Open the devcontainer and re-run, or run \`wtf cleanup\` inside it."
    echo "project ${DC_PROJECT} offline" >> "$SUM"
fi

# --------------------------------------------------------------------------
bold "── 2. Docker Desktop"
if [ "$APPLY" -eq 1 ]; then
    DOCKER_RECLAIM_SUMMARY=1 bash "$HERE/docker-reclaim.sh" --apply cache-dead dangling builders anon-volumes | relay docker
else
    DOCKER_RECLAIM_SUMMARY=1 bash "$HERE/docker-reclaim.sh" | relay docker
fi

# --------------------------------------------------------------------------
bold "── 3. Other running devcontainers"
if [ "$ALL" -ne 1 ]; then
    echo "  not included — add --all to sweep every other running devcontainer too"
else
    N=0
    for c in $(docker ps --format '{{.Names}}'); do
        [ "$c" = "$SELF" ] && continue
        img=$(docker inspect -f '{{.Image}}' "$c" 2>/dev/null)
        ver=$(docker image inspect -f '{{index .Config.Labels "org.stitchu.base.version"}}' "$img" 2>/dev/null)
        [ -n "$ver" ] || continue
        N=$((N + 1))
        sweep_in "$c"
    done
    [ "$N" -eq 0 ] && echo "  (none running)"
fi

# --------------------------------------------------------------------------
# The short version: one line per project, one for Docker, the total.
bold "── Summary"
VERB=$([ "$APPLY" -eq 1 ] && echo "freed" || echo "can free")
awk -v verb="$VERB" '
  function h(b) { return b >= 1e9 ? sprintf("%.1f GB", b/1e9) : (b >= 1e6 ? sprintf("%.0f MB", b/1e6) : sprintf("%.0f KB", b/1e3)) }
  function part(kv, k,   a, i, x) { n = split(kv, a, " "); for (i = 1; i <= n; i++) { split(a[i], x, "="); if (x[1] == k) return x[2] } return "0" }
  $1 == "project" {
    name = $2
    if ($3 == "offline") { printf "  %-28s not connected\n", "project " name; next }
    if ($3 == "skipped") { printf "  %-28s skipped (image < 1.12)\n", "project " name; next }
    rest = $0; sub(/^project [^ ]+ /, "", rest)
    split(part(rest, "tmp"), t, ":"); split(part(rest, "scratch"), sc, ":"); split(part(rest, "archive"), ar, ":")
    b = t[2] + sc[2] + ar[2]; total += b
    printf "  %-28s %9s  (%d test temp dirs, %d ended sessions, %d conversations %s)\n", "project " name, h(b), t[1], sc[1], ar[1], (verb == "freed" ? "archived" : "to archive")
    next
  }
  $1 == "docker" {
    rest = $0; sub(/^docker /, "", rest)
    c = part(rest, "cache"); split(part(rest, "dangling"), d, ":")
    b = c + d[2]; total += b
    printf "  %-28s %9s  (build cache %s, %d unused images %s, %d builders, %d anon volumes)\n", "docker", h(b), h(c), d[1], h(d[2]), part(rest, "builders"), part(rest, "anon")
  }
  END { printf "  %-28s %9s  %s\n", "total", h(total), verb }
' "$SUM"
echo
echo "  left alone, on purpose:"
echo "    $(docker volume ls -q | grep -cvE '^[0-9a-f]{64}$') named volume(s) — databases, Claude configs and conversations"
echo "    $(docker ps -aq --filter status=exited --filter status=created | wc -l | tr -d ' ') stopped container(s), $(docker images --format '{{.Repository}}' | grep -cv '<none>') tagged image(s), every running Claude session"
if [ "$APPLY" -eq 1 ]; then
    echo "  ✓ done — nothing in use was touched."
else
    echo "  ✓ safe to apply: what is listed is unused cache, leftovers nothing reads,"
    echo "    or conversations archived (restorable). Your data stays where it is."
fi
AFTER=$(vm_used || echo "")
if [ -n "$AFTER" ] && [ "$APPLY" -ne 1 ]; then
    echo "  VM disk used : $(gb "$AFTER")"
elif [ -n "$BEFORE" ] && [ -n "$AFTER" ]; then
    echo "  VM disk used : $(gb "$BEFORE") → $(gb "$AFTER")  (measured: $(gb $((BEFORE - AFTER))) freed)"
fi
if [ "$APPLY" -ne 1 ]; then
    echo
    echo "  nothing was removed — to do it:  $HINT"
fi
