#!/usr/bin/env bash
#
# .devcontainer/host-helpers/cleanup.sh — `wtf cleanup`: reclaim the Docker
# Desktop disk in one go, touching only what is safe to lose.
#
# Run on the HOST (macOS bash 3.2 compatible). Report only by default.
#
#   cleanup.sh            report: what each part would reclaim, nothing removed
#   cleanup.sh --apply    do it
#
# What --apply does:
#   1. Docker — docker-reclaim.sh --apply cache-dead dangling builders
#      anon-volumes: build cache backing no image, untagged images no
#      container uses, inactive buildx builders, anonymous volumes with zero
#      links.
#   2. Inside every RUNNING sandbox devcontainer (its image carries the
#      org.stitchu.base.version label): devc-sweep --apply as node —
#      abandoned test temp dirs, scratch dirs of ended Claude sessions, and
#      transcripts older than 30 days ARCHIVED (devc-sweep restore <id>).
#      Images older than 1.12 have no devc-sweep: listed as skipped.
#
# What it NEVER touches: named volumes (databases, Claude configs), stopped
# containers, tagged images, live Claude sessions. Stopped containers that pin
# images are listed by docker-reclaim's report for you to decide.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
APPLY=0
for a in "$@"; do
    case "$a" in
        --apply) APPLY=1 ;;
        -h|--help) sed -n '2,25p' "$0" | sed 's/^#\{1,\} \{0,1\}//'; exit 0 ;;
        *) echo "cleanup: unknown argument: $a (only --apply)" >&2; exit 64 ;;
    esac
done

# Inside a devcontainer `docker` is the nested daemon, not Docker Desktop:
# the report would describe the wrong disk. CLEANUP_ALLOW_NESTED exists for
# this script's own test only.
if [ -f /.dockerenv ] && [ "${CLEANUP_ALLOW_NESTED:-0}" != 1 ]; then
    echo "cleanup: run this on the Mac, not inside a devcontainer." >&2
    echo "  Here, \`docker\` talks to the nested daemon. To clean this container: devc-sweep" >&2
    exit 1
fi
docker info >/dev/null 2>&1 || { echo "cleanup: docker daemon unreachable" >&2; exit 1; }

bold() { printf '\n\033[1m%s\033[0m\n' "$1"; }

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
gb() { awk -v b="${1:-0}" 'BEGIN{printf "%.1f GB", b/1e9}'; }

printf '\033[1mwtf cleanup\033[0m  mode=%s\n' "$([ "$APPLY" -eq 1 ] && echo 'APPLY' || echo 'report only — nothing is removed')"
echo "  touches : build cache backing no image, untagged unused images, idle buildx"
echo "            builders, orphan anonymous volumes ; in each running devcontainer,"
echo "            abandoned temp dirs + ended sessions' scratch, transcripts > 30 days ARCHIVED"
echo "  never   : named volumes (databases, Claude configs), stopped containers,"
echo "            tagged images, live Claude sessions"
BEFORE=$(vm_used || echo "")

# --------------------------------------------------------------------------
bold "── 1. Docker"
if [ "$APPLY" -eq 1 ]; then
    bash "$HERE/docker-reclaim.sh" --apply cache-dead dangling builders anon-volumes
else
    bash "$HERE/docker-reclaim.sh"
fi

# --------------------------------------------------------------------------
bold "── 2. Inside the running devcontainers"
N=0
for c in $(docker ps --format '{{.Names}}'); do
    img=$(docker inspect -f '{{.Image}}' "$c" 2>/dev/null)
    ver=$(docker image inspect -f '{{index .Config.Labels "org.stitchu.base.version"}}' "$img" 2>/dev/null)
    [ -n "$ver" ] || continue
    N=$((N + 1))
    if ! docker exec "$c" test -x /usr/local/bin/devc-sweep 2>/dev/null; then
        printf '\n  %s (base %s) — skipped: image older than 1.12, no devc-sweep.\n' "$c" "$ver"
        echo "    Rebuild it on 1.12+ to get it."
        continue
    fi
    printf '\n\033[1m  %s\033[0m (base %s)\n' "$c" "$ver"
    TTYFLAG=""; [ -t 1 ] && TTYFLAG="-t"
    if [ "$APPLY" -eq 1 ]; then
        docker exec $TTYFLAG -u node "$c" devc-sweep --apply | sed 's/^/    /'
    else
        docker exec $TTYFLAG -u node "$c" devc-sweep | sed 's/^/    /'
    fi
done
[ "$N" -eq 0 ] && echo "  (no running sandbox devcontainer)"

# --------------------------------------------------------------------------
bold "── Summary"
AFTER=$(vm_used || echo "")
if [ -n "$AFTER" ] && [ "$APPLY" -ne 1 ]; then
    echo "  VM disk used : $(gb "$AFTER")"
elif [ -n "$BEFORE" ] && [ -n "$AFTER" ]; then
    echo "  VM disk used : $(gb "$BEFORE") → $(gb "$AFTER")  (freed $(gb $((BEFORE - AFTER))))"
else
    echo "  VM disk used : unknown (no local image could run df)"
fi
if [ "$APPLY" -ne 1 ]; then
    echo
    echo "  nothing was removed — to do all of the above:  wtf cleanup --apply"
fi
