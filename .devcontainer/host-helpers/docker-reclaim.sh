#!/usr/bin/env bash
#
# .devcontainer/host-helpers/docker-reclaim.sh — remove Docker junk that is
# genuinely junk, and nothing else.
#
# Run on the HOST. Sibling of docker-audit.sh, which measures THIS project's
# images; this one sweeps the whole daemon for reclaimable space.
#
#   docker-reclaim.sh                     report only — the default, removes nothing
#   docker-reclaim.sh --cache             add the build cache broken down by build step
#   docker-reclaim.sh --apply <targets…>  actually remove
#
# Targets — combine freely; --apply with none of them does nothing:
#
#   builders      inactive buildx builders (container driver) and their state
#                 volumes. No data: a builder is recreated on demand.
#   dangling      untagged images no container uses.
#   anon-volumes  ANONYMOUS volumes (64 hex chars) with zero links — what is
#                 left when a container that declared a VOLUME is deleted.
#   cache-dupes   Keeps the most recent record of each identical step and
#                 prunes the older copies by ID. Safe, but MEASURED LARGELY
#                 INEFFECTIVE: 328 records asked, 51 freed, 2.34 GB. The cache
#                 is a DAG — 98% of records have a parent — and the newest copy
#                 of a step usually descends from the older ones, so the copies
#                 this targets are the parents of the copies it keeps, and
#                 BuildKit refuses to drop them. `Reclaimable: true` means "not
#                 in use by a running build", NOT "removable on its own": a
#                 record matching that description pruned alone returns
#                 `Total: 0B`. Use cache-dead to actually reclaim the space.
#   cache-image   delete the cache of images you know are junk — your own test
#                 builds, say. Needs --images <tag,tag,…>. A record goes only
#                 if EVERY image whose history contains that step is in your
#                 list: 72% of steps are shared between images (measured), so
#                 "the cache of t220" naively would take devcontainer-base's
#                 cache with it. A record whose step matches no image at all is
#                 never deleted either — unknown means keep, always.
#   cache-dead    NO LIMIT, and the one that actually works. Drops every chain
#                 backing no existing image — exactly the "reclaimable" figure
#                 docker system df prints. Because it takes whole chains rather
#                 than cherry-picking DAG nodes, it reclaims what cache-dupes
#                 cannot. Cost: the next build re-runs those steps. Prefer it.
#   cache-trim    the blunt last resort: cap the whole cache at CACHE_KEEP
#                 (default 20GB), evicting LEAST RECENTLY USED first — this one
#                 WILL drop cache an existing image still shares.
#
#   CACHE_KEEP=30GB docker-reclaim.sh --apply cache-dupes dangling
#
# Stopped containers are deliberately NOT a target: a stopped project is still
# a project. Removing one loses its writable layer and its anonymous volumes.
#
# What it will NEVER remove, whatever you ask
# -------------------------------------------
# The difference between a cache and a database is a name you have to read, so
# every NAMED volume is out of scope — no exceptions, no flag. That covers
# claude-code-config-* (Claude sessions and transcripts), claude-creds-*,
# vscode, dind-storage-*, and every project database. Only anonymous volumes
# with zero links are ever candidates.
set -u

CACHE_KEEP="${CACHE_KEEP:-20GB}"
IMAGES=""

APPLY=0
CACHE_REPORT=0
WANT=" "
for a in "$@"; do
    case "$a" in
        --apply)   APPLY=1 ;;
        --cache)   CACHE_REPORT=1 ;;
        --images=*) IMAGES="${a#--images=}" ;;
        --images)  echo "docker-reclaim: --images wants =tag,tag" >&2; exit 64 ;;
        -h|--help) sed -n '2,48p' "$0" | sed 's/^#\{1,\} \{0,1\}//'; exit 0 ;;
        --*)       echo "docker-reclaim: unknown option: $a" >&2; exit 64 ;;
        builders|dangling|anon-volumes|cache-dupes|cache-dead|cache-image|cache-trim) WANT="$WANT$a " ;;
        *)         echo "docker-reclaim: unknown target: $a" >&2; exit 64 ;;
    esac
done
want() { case "$WANT" in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT

# Progress goes to stderr, and only on a terminal: every docker call below can
# sit silent for minutes on a loaded daemon (a full disk makes it worse), and
# silence reads as "hung". `tick` overwrites one line ; `untick` clears it.
TTY=0; [ -t 2 ] && TTY=1
tick()   { [ "$TTY" -eq 1 ] && printf '\r\033[K  … %s' "$1" >&2; return 0; }
untick() { [ "$TTY" -eq 1 ] && printf '\r\033[K' >&2; return 0; }

# wait_on <label> <outfile> <cmd…> — run a long, silent command in the
# background with a live line: elapsed seconds, and how many lines it has
# written so far (for a prune, that is the records deleted). Returns its exit
# status ; its output is in <outfile>.
wait_on() {
    local label="$1" out="$2" pid t rc start=$SECONDS; shift 2
    "$@" > "$out" 2>&1 &
    pid=$!
    # Polled every 0.2 s, not 1 s: most calls are quick, and a whole-second
    # sleep would add a second to each of them.
    while kill -0 "$pid" 2>/dev/null; do
        tick "$label — $((SECONDS - start))s, $(wc -l < "$out" | tr -d ' ') lines so far"
        sleep 0.2
    done
    wait "$pid"; rc=$?
    t=$((SECONDS - start))
    untick
    [ "$t" -ge 3 ] && printf '  (%s took %ds)\n' "$label" "$t" >&2
    return "$rc"
}

wait_on "contacting the docker daemon" "$TMP/info" docker info \
    || { echo "docker daemon unreachable — run this on the host" >&2; exit 1; }

hr()  { printf '\n\033[1m── %s\033[0m\n' "$1"; }
act() { if [ "$APPLY" -eq 1 ]; then printf '  removing      %s\n' "$1"; else printf '  would remove  %s\n' "$1"; fi; }
gb()  { awk -v b="${1:-0}" 'BEGIN{printf "%.2fG", b/1000000000}'; }
# Human sizes as docker prints them (143kB, 1.25GB, 0B) → bytes, on stdin.
TOBYTES='function tob(s,  u,v) { u=s; sub(/^[0-9.]+/,"",u); v=s+0
  if (u=="kB"||u=="KB") return v*1e3; if (u=="MB") return v*1e6
  if (u=="GB") return v*1e9; if (u=="TB") return v*1e12; return v }'

# The disk the daemon writes to, as the VM sees it: df inside a throwaway
# container of an image already present (no pull, no network). Empty when no
# local image has a df, or when the disk is too full to start a container.
vm_disk() {
    local img
    for img in $(docker images --format '{{.ID}}' | head -5); do
        docker run --rm --network none --entrypoint df "$img" -k / 2>/dev/null \
            | awk 'NR==2 { printf "%.0f\t%.0f\t%.0f\n", $2*1024, $3*1024, $4*1024; ok=1 } END { exit !ok }' \
            && return 0
    done
    return 1
}

# Totals: what the disk holds, what docker accounts for, and the gap — the
# figure `docker system df` never prints, and the one that says whether a
# prune can help at all.
disk_totals() {
    wait_on "measuring the VM disk" "$TMP/vm" vm_disk
    docker system df --format '{{.Type}}\t{{.Size}}\t{{.Reclaimable}}' > "$TMP/dft" 2>/dev/null
    awk -F'\t' -v nested="$([ -f /.dockerenv ] && echo 1 || echo 0)" "$TOBYTES"'
      FILENAME ~ /vm$/ { size=$1; used=$2; free=$3; vm=1; next }
      { b=tob($2); r=$3; sub(/ .*/,"",r); rb=tob(r); acc+=b; rec+=rb
        printf "  %-16s %8.2f GB   reclaimable %8.2f GB\n", $1, b/1e9, rb/1e9 }
      END {
        printf "  %-16s %8.2f GB   reclaimable %8.2f GB\n", "TOTAL docker", acc/1e9, rec/1e9
        if (vm) {
          printf "  %-16s %8.2f GB used of %.2f GB, %.2f GB free (%.0f%%)\n", "VM disk", used/1e9, size/1e9, free/1e9, used*100/size
          gap = used - acc
          if (gap > 1e9) printf "  %-16s %8.2f GB   on the disk but in no docker figure — with the containerd image store,\n                                  mostly the compressed copy of every image ; then container logs\n", "unaccounted", gap/1e9
          if (nested) print "  (nested daemon: its disk is the host daemon\x27s, so \"unaccounted\" is mostly the host\x27s own content)"
        } else print "  VM disk          unknown (no local image with df, or the disk is too full to start a container)"
      }' "$TMP/vm" "$TMP/dft"
}

# Prune cache records one id at a time, and report what was ACTUALLY freed.
# `docker builder prune` exits 0 even when it frees nothing, so trusting its
# exit status made this script claim 23.70 G while the daemon totals moved by
# 2.34 G. The only truthful signal is the `Total:` line it prints, so that is
# what gets summed — and records that freed 0 are counted separately, because
# a large gap between asked and hit is the interesting number.
prune_ids() {
    local file="$1" total="$2" id sz out got i=0 hit=0 freed=0
    while IFS="$(printf '\t')" read -r id sz _; do
        i=$((i + 1))
        out="$(docker builder prune -f --filter "id=$id" 2>/dev/null)"
        got="$(printf '%s' "$out" | awk '/^Total:/ {
            n=$2; u=n; sub(/^[0-9.]+/,"",u); v=n+0
            if (u=="kB") print int(v*1000); else if (u=="MB") print int(v*1000000)
            else if (u=="GB") print int(v*1000000000); else print int(v); exit }')"
        if [ -n "${got:-}" ] && [ "$got" -gt 0 ] 2>/dev/null; then
            hit=$((hit + 1)); freed=$((freed + got))
        fi
        tick "pruning record $i/$total — $hit freed space so far"
    done < "$file"
    untick
    printf '    asked %d records, %d actually freed space, %s reclaimed\n' "$i" "$hit" "$(gb "$freed")"
    [ "$hit" -lt "$i" ] && printf '    (%d freed nothing: already gone with a parent, or still referenced)\n' "$((i - hit))"
    return 0
}

printf '\033[1mdocker-reclaim\033[0m  mode=%s\n' "$([ "$APPLY" -eq 1 ] && echo APPLY || echo 'report only')"
wait_on "docker system df" "$TMP/df" docker system df; cat "$TMP/df"

# `docker system df -v` sizes every volume, and is the slowest call here. It
# used to run once per builder and once per orphaned state volume ; read it
# once. Sizes are only displayed, so a snapshot taken before removals is fine.
wait_on "docker system df -v (sizing every volume)" "$TMP/dfv" docker system df -v

# --------------------------------------------------------------------------
hr "where the disk goes"
disk_totals
# Named volumes are never a target, so when they hold the space no prune will
# help — say so with the numbers instead of letting "nothing reclaimable" read
# as a mystery. dind-storage-* is a devcontainer's nested daemon: clean it
# from INSIDE that devcontainer, where `docker` talks to it.
echo
echo "  largest volumes (never removed here — named volumes are data):"
awk "$TOBYTES"'
  /^VOLUME NAME/ { f=1; next } /^$/ { f=0 }
  f && NF>=3 { printf "%.0f\t%s\t%s\n", tob($NF), $NF, $1 }' "$TMP/dfv" \
  | sort -rn | head -8 | awk -F'\t' '{ printf "    %10s  %s%s\n", $2, $3, ($3 ~ /^dind-storage-/ ? "   ← nested docker: run wtf docker reclaim inside that devcontainer" : "") }'
echo
echo "  largest container writable layers (stopped containers are kept):"
wait_on "sizing containers" "$TMP/ps" docker ps -a --size --format '{{.Names}}\t{{.Status}}\t{{.Size}}'
awk -F'\t' "$TOBYTES"'{ s=$3; sub(/ .*/,"",s); printf "%.0f\t%s\t%s\t%s\n", tob(s), s, $1, $2 }' "$TMP/ps" \
  | sort -rn | head -5 | awk -F'\t' '{ printf "    %10s  %s (%s)\n", $2, $3, $4 }'
echo "  (a container's size is its own writable layer only — the \"virtual\" part"
echo "   is its image, counted once under Images, never twice)"

# An image any container still references cannot be removed — stopped ones
# included. So old stopped containers are what keeps "reclaimable" at zero
# while the images pile up. Advisory: it lists, it never removes a container.
echo
echo "  images held ONLY by stopped containers (removing those containers frees up to):"
docker ps -aq > "$TMP/cids"
if [ -s "$TMP/cids" ]; then
    wait_on "inspecting containers" "$TMP/cinsp" \
        sh -c 'docker inspect --format "{{.Image}}	{{.Name}}	{{.State.Running}}" $(cat "$1")' _ "$TMP/cids"
    awk -F'\t' '{ n=$2; sub(/^\//,"",n)
                  if ($3=="true") run[$1]=1; else held[$1]=held[$1] (held[$1]==""?"":", ") n }
         END { for (i in held) if (!(i in run)) print i"\t"held[i] }' "$TMP/cinsp" > "$TMP/held"
fi
if [ -s "$TMP/held" ]; then
    docker image inspect --format '{{.Id}}	{{.Size}}	{{join .RepoTags ","}}' $(cut -f1 "$TMP/held") 2>/dev/null > "$TMP/heldsz"
    awk -F'\t' 'NR==FNR { sz[$1]=$2; tag[$1]=($3==""?"<none>":$3); next }
         { printf "%.0f\t%s\t%s\t%s\n", sz[$1], substr($1,8,12), tag[$1], $2 }' "$TMP/heldsz" "$TMP/held" \
      | sort -rn | awk -F'\t' '{ t+=$1; if (NR<=12) printf "    %7.2fG  %s %-28.28s ← %.70s\n", $1/1e9, $2, $3, $4 }
          END { printf "    total up to %.2f GB over %d images (shared base layers count once, so less in practice)\n", t/1e9, NR }'
    echo "    a random two-word name (quirky_brattain) is a docker run throwaway ; a"
    echo "    compose name (<project>-app-1) is a project — remove by hand, if at all:"
    echo "      docker rm <name> …   then re-run with --apply dangling"
else
    echo "    (none)"
fi

# --------------------------------------------------------------------------
hr "inactive buildx builders"
N=0
tick "listing buildx builders"
for b in $(docker buildx ls --format '{{.Name}}' 2>/dev/null | grep -v '^NAME' | sed 's/\*$//'); do
    case "$b" in default|desktop-linux) continue ;; esac
    tick "inspecting builder $b"
    docker buildx inspect "$b" 2>/dev/null | grep -q 'Driver: *docker-container' || continue
    VOL="buildx_buildkit_${b}0_state"
    SZ=$(awk -v v="$VOL" '$1==v{print $NF}' "$TMP/dfv")
    untick; N=$((N + 1)); act "builder $b (state ${SZ:-?})"
    if [ "$APPLY" -eq 1 ] && want builders; then
        docker buildx rm "$b" >/dev/null 2>&1 && echo "    ✔ builder gone" || echo "    ✘ failed"
        # `buildx rm` on an ALREADY-INACTIVE builder leaves its state volume
        # behind — measured: four builders removed, 13.44 GB of state still on
        # disk, and now unreachable because the names that addressed it are
        # gone. So the volume is removed explicitly, after the builder.
        if docker volume inspect "$VOL" >/dev/null 2>&1; then
            docker volume rm "$VOL" >/dev/null 2>&1 \
                && echo "    ✔ state volume gone (${SZ:-?})" \
                || echo "    ✘ state volume $VOL still in use"
        fi
    fi
done
untick
[ "$N" -eq 0 ] && echo "  (none)"

# Orphans from an earlier run, or from a `buildx rm` done by hand: a state
# volume whose builder no longer exists can never be reached again.
for v in $(docker volume ls --format '{{.Name}}' | grep '^buildx_buildkit_'); do
    b="${v#buildx_buildkit_}"; b="${b%0_state}"
    tick "checking orphaned state volume $v"
    docker buildx inspect "$b" >/dev/null 2>&1 && continue
    untick
    SZ=$(awk -v x="$v" '$1==x{print $NF}' "$TMP/dfv")
    N=$((N + 1)); act "orphaned builder state $v (${SZ:-?}) — its builder is gone"
    if [ "$APPLY" -eq 1 ] && want builders; then
        docker volume rm "$v" >/dev/null 2>&1 && echo "    ✔ gone" || echo "    ✘ in use"
    fi
done
untick
N_BUILDERS=$N

# --------------------------------------------------------------------------
hr "dangling images with no container"
N=0
tick "listing dangling images"
for id in $(docker images -f "dangling=true" -q); do
    tick "checking dangling image $id"
    USED=$(docker ps -a --filter "ancestor=$id" --format '{{.Names}}' | head -1)
    untick
    [ -n "$USED" ] && { printf '  kept          %s (used by %s)\n' "$id" "$USED"; continue; }
    N=$((N + 1))
    act "image $id ($(gb "$(docker image inspect "$id" -f '{{.Size}}' 2>/dev/null)"))"
    if [ "$APPLY" -eq 1 ] && want dangling; then
        docker rmi "$id" >/dev/null 2>&1 && echo "    ✔ gone" || echo "    ✘ in use"
    fi
done
untick
[ "$N" -eq 0 ] && echo "  (none)"
N_DANGLING=$N

# --------------------------------------------------------------------------
hr "anonymous volumes with zero links"
N=0
awk '/^VOLUME NAME/{f=1;next} /^$/{f=0} f && NF>=3 {print $1"\t"$(NF-1)"\t"$NF}' "$TMP/dfv" > "$TMP/vols"
while IFS=$'\t' read -r name links size; do
    [ "${links:-1}" = "0" ] || continue
    printf '%s' "$name" | grep -qE '^[0-9a-f]{64}$' || continue
    N=$((N + 1)); act "volume $(printf '%.12s' "$name")… ($size)"
    if [ "$APPLY" -eq 1 ] && want anon-volumes; then
        docker volume rm "$name" >/dev/null 2>&1 && echo "    ✔ gone" || echo "    ✘ in use"
    fi
done < "$TMP/vols"
[ "$N" -eq 0 ] && echo "  (none)"
echo "  named volumes are never candidates — see the header"
N_ANONVOL=$N

# --------------------------------------------------------------------------
# BuildKit does not record which image a cache entry belongs to. But every
# image keeps its layer commands in `docker history`, and a cache entry's
# Description IS that command — so the two can be joined on the command text.
#
# The join was verified deterministic: two independent implementations produce
# byte-identical keys on both sides, over every record. An earlier count that
# seemed to disagree by 123 was a difference in how the buckets were tallied,
# not in the matching.
#
# INVARIANT — only cache-image consumes this, and only under two fail-closed
# rules, because 72% of steps were measured shared between several images:
#   1. a record whose step matches NO image history is never deleted;
#   2. a record is deleted only if EVERY image owning its step was named.
# So an incomplete join costs disk space, never someone else's cache. Any new
# use of this data must keep both rules. cache-dupes deliberately does NOT read
# it: it keeps the newest record of each step, which is safe on its own.
# Both sides are reduced to the same key: drop the `RUN |N <args>` / `mount /
# from exec` / `[N/M]` prefixes and the ` # buildkit` suffix, then collapse
# whitespace. `{{json .CreatedBy}}` is what makes it work at all: a RUN written
# across continuation lines is multi-line, and would otherwise break any
# line-oriented pipeline.
normkey() {
    awk '{
        s = $0
        sub(/[[:space:]]+#[[:space:]]*buildkit$/, "", s)
        if (s ~ /^RUN /) { p = index(s, "/bin/sh -c "); if (p > 0) s = substr(s, p + 11) }
        else if (s ~ /^mount \/ from exec /) { p = index(s, "/bin/sh -c "); if (p > 0) s = substr(s, p + 11) }
        sub(/^\[[^]]*\][[:space:]]*/, "", s)
        gsub(/[[:space:]]+/, " ", s); sub(/^ /, "", s); sub(/ $/, "", s)
        print s
    }'
}

: > "$TMP/histmap"
docker images --format '{{.Repository}}:{{.Tag}}' | grep -v '<none>' > "$TMP/tags"
NT=$(wc -l < "$TMP/tags" | tr -d ' '); IT=0
for t in $(cat "$TMP/tags"); do
    IT=$((IT + 1)); tick "reading image histories $IT/$NT — $t"
    docker history --no-trunc --format '{{json .CreatedBy}}' "$t" 2>/dev/null \
      | sed -e 's/^"//; s/"$//' -e 's/\\n/ /g; s/\\t/ /g; s/\\"/"/g; s/\\\\/\\/g' \
      | normkey \
      | awk -v img="$t" 'length($0) > 8 { print $0"\t"img }' >> "$TMP/histmap"
done
untick
sort -u -o "$TMP/histmap" "$TMP/histmap"

# One TSV line per cache record. Tabs are stripped from the description so it
# can be the grouping key. A record is a candidate only when it is reclaimable
# AND not shared with an image.
wait_on "docker builder du --verbose (listing every cache record)" "$TMP/du" docker builder du --verbose
awk '
  function bytes(n,   u,v) {
    u=n; sub(/^[0-9.]+/,"",u); v=n+0
    if (u=="kB") return v*1000; if (u=="MB") return v*1000000
    if (u=="GB") return v*1000000000; if (u=="TB") return v*1000000000000
    return v
  }
  /^ID:/           { id=$2 }
  /^Created at:/   { created=$3" "$4 }
  /^Reclaimable:/  { rec=$2 }
  /^Shared:/       { shr=$2 }
  /^Size:/         { sz=bytes($2) }
  /^Description:/  { $1=""; d=substr($0,2); gsub(/\t/," ",d); gsub(/[[:space:]]+$/,"",d); desc=d }
  /^$/ {
    # %.0f, not %d: mawk'\''s %d clamps any value over 2147483647 (2.147 GB) to
    # that ceiling — common for a single cache record — silently shrinking
    # every size past it. %.0f formats the same double without the C-int cap.
    if (id != "") printf "%s\t%s\t%s\t%.0f\t%s\t%s\n", desc, created, id, sz, shr, rec
    id=""; desc=""; created=""; sz=0; shr=""; rec=""
  }
'  "$TMP/du" > "$TMP/cache6" 2>/dev/null || true

# Column 7 is the join key, produced by piping column 1 through the SAME
# normkey() the history side used. Duplicating that logic in a second awk is
# how the two sides came to disagree — 863 unattached by one, 741 by the other.
# One implementation, `paste`d back on, cannot drift.
cut -f1 "$TMP/cache6" | normkey > "$TMP/cachekey"
paste "$TMP/cache6" "$TMP/cachekey" > "$TMP/cache"

# key -> comma-separated image list
awk -F'\t' '{ if ($1 in m) m[$1]=m[$1]","$2; else m[$1]=$2 }
            END { for (k in m) print k"\t"m[k] }' "$TMP/histmap" > "$TMP/owner"

hr "build cache"
if [ ! -s "$TMP/cache" ]; then
    echo "  (no cache records)"
    CACHE_DEAD_N=0; CACHE_DEAD_B=0
else
    # Same filter as the "backing nothing that exists" row above, kept in
    # shell variables so the final "recommended" section can quote the exact
    # figure instead of making the reader re-read the table.
    CACHE_DEAD_N=$(awk -F'\t' '$6=="true" && $5!="true"{n++} END{print n+0}' "$TMP/cache")
    CACHE_DEAD_B=$(awk -F'\t' '$6=="true" && $5!="true"{t+=$4} END{printf "%.0f", t+0}' "$TMP/cache")
    # Shared = a layer an existing image still points at. Unshared+reclaimable
    # is exactly the "RECLAIMABLE" number docker system df prints for the cache.
    awk -F'\t' '
      { n++; tot+=$4
        if ($6 != "true")      { iu++;  iub+=$4 }
        else if ($5 == "true") { sh++;  shb+=$4 }
        else                   { un++;  unb+=$4 }
      }
      END {
        printf "  %d records, %.2f GB total\n", n, tot/1000000000
        printf "    backing an existing image (kept by every target) : %5d  %6.2f GB\n", sh+0, shb/1000000000
        printf "    backing nothing that exists  → cache-dead        : %5d  %6.2f GB\n", un+0, unb/1000000000
        if (iu+0 > 0) printf "    in use by a running build (never touched)       : %5d  %6.2f GB\n", iu, iub/1000000000
      }' "$TMP/cache"

    # Attribution — ADVISORY ONLY, it removes nothing. Two whole classes of
    # record can never match a history and are not evidence of anything:
    # context transfers (`local source for…`, `from local`) have no Dockerfile
    # line, and a step from an intermediate build stage leaves no layer in the
    # final image. Counting either as "dead" is how you delete live cache.
    awk -F'\t' 'NR==FNR { own[$1]=1; next }
      {
        if ($1 ~ /^(local source|from local|pulled from)/) { nm++; nmb+=$4 }
        else if ($1 ~ /^\[[^]]*[a-z-]+ [0-9]+\/[0-9]+\]/) { st++; stb+=$4 }
        else if ($7 in own) { a++; ab+=$4 }
        else { o++; ob+=$4 }
      }
      END {
        printf "    step still in some image history                : %5d  %6.2f GB\n", a+0, ab/1000000000
        printf "    unmatchable by construction (context, stages)   : %5d  %6.2f GB\n", nm+st+0, (nmb+stb)/1000000000
        printf "    step in NO image (advisory — deletes nothing)   : %5d  %6.2f GB\n", o+0, ob/1000000000
      }' "$TMP/owner" "$TMP/cache"

    # Group by identical build step, newest first, and mark every copy after
    # the first as superseded.
    sort -t"$(printf '\t')" -k1,1 -k2,2r "$TMP/cache" | awk -F'\t' '
      { if ($1 != prev) { prev=$1; rank=1 } else { rank++ }
        if (rank > 1 && $5 == "false" && $6 == "true") print $3"\t"$4"\t"$1 }
    ' > "$TMP/dupes"
    awk -F'\t' '{n++; tot+=$2} END{
      printf "  of which %d superseded copies of a step already cached more recently: %.2f GB\n",
             n+0, tot/1000000000 }' "$TMP/dupes"

    # Cache whose step belongs to ONE image and no other — the only cache that
    # can be attributed to an image without taking a neighbour's with it.
    echo
    echo "  cache exclusive to a single image (what cache-image can target):"
    awk -F'\t' 'NR==FNR { own[$1]=$2; next }
      { if (!($7 in own)) next
        o=own[$7]; if (o ~ /,/) next
        ex[o]+=$4; n[o]++ }
      END { if (length(ex)==0) print "    (none — every attributed step is shared)"
            for (i in ex) printf "    %-46s %5d records  %6.2f GB\n", i, n[i], ex[i]/1000000000 }
      ' "$TMP/owner" "$TMP/cache" | sort -k3 -rn | head -12

    if [ "$CACHE_REPORT" -eq 1 ]; then
        echo
        sort -t"$(printf '\t')" -k1,1 -k2,2r "$TMP/cache" | awk -F'\t' '
          NR==FNR { own[$1]=$2; next }
          { tot[$1]+=$4; cnt[$1]++; key[$1]=$7; if (!($1 in newest)) newest[$1]=$2 }
          END { for (d in tot) {
                  if (key[d] in own) who = own[key[d]]
                  else if (d ~ /^(local source|from local|pulled from)/) who = "n/a · context or pull"
                  else if (d ~ /^\[[^]]*[a-z-]+ [0-9]+\/[0-9]+\]/)       who = "n/a · build stage"
                  else if (d ~ /^[[:space:]]*$/)                          who = "n/a · no step text"
                  else who = "— none —"
                  printf "%d\t%d\t%s\t%s\t%s\n", tot[d], cnt[d], substr(newest[d],1,10), who,
                         (d ~ /^[[:space:]]*$/ ? "(empty description)" : d) } }
        ' "$TMP/owner" - | sort -rn | head -25 | awk -F'\t' '
          BEGIN { printf "  %9s %6s  %-11s %-34s %s\n", "TOTAL", "COPIES", "NEWEST", "IMAGE(S)", "BUILD STEP" }
          # Never truncate a comma list into looking like a single owner: a
          # shared step read as "belongs to barecheck" is how you delete the
          # cache of a neighbour. Say how many own it instead.
          # (No apostrophes in here — the awk program is single-quoted.)
          { img=$4
            n=split(img, parts, ",")
            if (n > 1) { img = parts[1]; if (length(img) > 20) img = substr(img,1,19) "…"
                         img = img " +" (n-1) " more" }
            else if (length(img) > 33) img = substr(img,1,32) "…"
            printf "  %8.2fG %6s  %-11s %-34s %.44s\n", $1/1000000000, $2, $3, img, $5 }
          END { print "\n  IMAGE(S) is recovered by joining the step text against every image'\''s\n  docker history — BuildKit itself does not record it. \"— none —\" means\n  the step survives in no image at all, so that cache can never be hit.\n  COPIES > 1 is the same line re-cached on every rebuild." }'
    fi

    if [ "$APPLY" -eq 1 ] && want cache-dupes; then
        echo
        TOTN=$(wc -l < "$TMP/dupes" | tr -d ' ')
        echo "  pruning $TOTN superseded records by ID (one call each, this is slow)…"
        prune_ids "$TMP/dupes" "$TOTN"
    fi

    if [ "$APPLY" -eq 1 ] && want cache-image; then
        echo
        if [ -z "$IMAGES" ]; then
            echo "  ✘ cache-image needs --images=tag,tag — refusing to guess" >&2
        else
            # Fail closed, twice: a record with no attribution is kept, and a
            # record any image OUTSIDE the list also owns is kept.
            awk -F'\t' -v want="$IMAGES" '
              BEGIN { nw=split(want, a, ","); for (i=1;i<=nw;i++) { gsub(/^ +| +$/,"",a[i]); W[a[i]]=1 } }
              NR==FNR { own[$1]=$2; next }
              { if (!($7 in own)) next
                if ($6 != "true") next
                n=split(own[$7], o, ","); for (i=1;i<=n;i++) if (!(o[i] in W)) next
                print $3"\t"$4"\t"own[$7] }
            ' "$TMP/owner" "$TMP/cache" > "$TMP/byimage"
            N=$(wc -l < "$TMP/byimage" | tr -d " ")
            awk -F'\t' '{t+=$2} END{printf "  %d records exclusive to [%s], about %.2f GB\n", NR, "'"$IMAGES"'", t/1000000000}' "$TMP/byimage"
            prune_ids "$TMP/byimage" "$N"
        fi
    fi

    if [ "$APPLY" -eq 1 ] && want cache-dead; then
        echo
        echo "  pruning every record that backs no existing image (no size limit)…"
        wait_on "docker builder prune" "$TMP/prune" docker builder prune -f
        tail -2 "$TMP/prune"
    fi

    if [ "$APPLY" -eq 1 ] && want cache-trim; then
        echo
        echo "  trimming the whole cache to $CACHE_KEEP, least recently used first…"
        wait_on "docker builder prune --max-used-space=$CACHE_KEEP" "$TMP/prune" \
            docker builder prune -f --max-used-space="$CACHE_KEEP"
        tail -2 "$TMP/prune"
    fi
fi

hr "after"
disk_totals

# --------------------------------------------------------------------------
# What to actually run — spelled out, because the tables above require doing
# arithmetic across three sections to answer that. Only targets that found
# something are listed; an empty target is omitted rather than shown at 0.
if [ "$APPLY" -ne 1 ]; then
    hr "recommended"
    PRINTED=0
    [ "${N_BUILDERS:-0}" -gt 0 ] && { printf '  builders       %-3d found           →  %s --apply builders\n' "$N_BUILDERS" "$(basename "$0")"; PRINTED=1; }
    [ "${N_DANGLING:-0}" -gt 0 ] && { printf '  dangling       %-3d found           →  %s --apply dangling\n' "$N_DANGLING" "$(basename "$0")"; PRINTED=1; }
    [ "${N_ANONVOL:-0}" -gt 0 ] && { printf '  anon-volumes   %-3d found           →  %s --apply anon-volumes\n' "$N_ANONVOL" "$(basename "$0")"; PRINTED=1; }
    [ "${CACHE_DEAD_N:-0}" -gt 0 ] && { printf '  cache-dead     %-8s (%d records)  →  %s --apply cache-dead\n' "$(gb "${CACHE_DEAD_B:-0}")" "$CACHE_DEAD_N" "$(basename "$0")"; PRINTED=1; }
    if [ "$PRINTED" -eq 1 ]; then
        echo
        echo "  nothing was removed — combine any of the above in one --apply call"
    else
        echo "  nothing worth removing right now"
    fi
fi
