#!/usr/bin/env bash
#
# .devcontainer/host-helpers/docker-test-images.sh — remove the TAGGED images
# our own test runs left behind, and nothing else.
#
# Run on the HOST. docker-reclaim.sh deliberately never touches a tagged image:
# it cannot know which tag is disposable. This one does, because the tags come
# from this repo's own test plans — it matches nothing it was not taught.
#
#   docker-test-images.sh            report only — the default, removes nothing
#   docker-test-images.sh --all      also list the images it will NEVER touch
#   docker-test-images.sh --apply    actually remove the candidates
#
# Order matters. Removing an image does not free its build cache — the cache
# chain simply stops backing anything. So:
#
#   docker-test-images.sh --apply
#   docker-reclaim.sh --apply dangling cache-dead
#
# the second call is what turns the freed images into actual disk space.
#
# Candidate rules — a tag must match one of these to be proposed at all:
#
#   *smoke*        the smoke builds: devcontainer-base:smoke-multi,
#                  claude-devcontainer-base:smoke-v3, v3-smoke:{optin,hardened}
#   *test*         one-off stack probes: claude-devcontainer:php-test
#   t<digits>      a build tagged after the test-plan row that asked for it
#   cc<digits>     a build tagged after a Claude Code version under test
#
# Never a candidate, whatever it matches
# --------------------------------------
#   - any image a container references, running OR stopped. A stopped project
#     is still a project (same rule as docker-reclaim.sh).
#   - anything in KEEP below: the images the devcontainer actually boots from.
#   - anything matching no rule. Unknown means keep, always.
set -u

KEEP="devcontainer-base:local devcontainer-sandbox:local devcontainer-sandbox:cc"

APPLY=0
SHOW_ALL=0
for a in "$@"; do
    case "$a" in
        --apply)   APPLY=1 ;;
        --all)     SHOW_ALL=1 ;;
        -h|--help) sed -n '2,38p' "$0" | sed 's/^#\{1,\} \{0,1\}//'; exit 0 ;;
        *)         echo "docker-test-images: unknown option: $a" >&2; exit 64 ;;
    esac
done

docker info >/dev/null 2>&1 || { echo "docker daemon unreachable — run this on the host" >&2; exit 1; }

gb() { awk -v b="${1:-0}" 'BEGIN{printf "%.2fG", b/1000000000}'; }

# One pass over the containers: an image referenced by any of them is out of
# scope. `ancestor=` per image would be one daemon call per tag, and misses a
# container created from a tag that has since moved.
USED="$(docker ps -a --format '{{.Image}}' | sort -u)"
used() { printf '%s\n' "$USED" | grep -qxF "$1"; }
kept() { case " $KEEP " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

candidate() {
    case "$1" in
        *smoke*|*test*) return 0 ;;
    esac
    # t220, cc268:local — the repo tag or the image tag, digits mandatory so
    # `tools` or `ccache` never qualify.
    printf '%s' "$1" | grep -qE '(^|/)(t|cc)[0-9]+(:|$)' && return 0
    return 1
}

printf '\033[1mdocker-test-images\033[0m  mode=%s\n\n' "$([ "$APPLY" -eq 1 ] && echo APPLY || echo 'report only')"

TOTAL=0
N=0
while IFS="$(printf '\t')" read -r ref size; do
    [ "${ref##*:}" = "<none>" ] && continue
    case "$ref" in '<none>'*) continue ;; esac

    if kept "$ref"; then
        [ "$SHOW_ALL" -eq 1 ] && printf '  kept      %-46s %8s  (boot image)\n' "$ref" "$(gb "$size")"
        continue
    fi
    if ! candidate "$ref"; then
        [ "$SHOW_ALL" -eq 1 ] && printf '  kept      %-46s %8s  (matches no test rule)\n' "$ref" "$(gb "$size")"
        continue
    fi
    if used "$ref"; then
        printf '  kept      %-46s %8s  (a container uses it)\n' "$ref" "$(gb "$size")"
        continue
    fi

    N=$((N + 1)); TOTAL=$((TOTAL + size))
    if [ "$APPLY" -eq 1 ]; then
        printf '  removing  %-46s %8s\n' "$ref" "$(gb "$size")"
        docker rmi "$ref" >/dev/null 2>&1 && echo "    ✔ gone" || echo "    ✘ refused — another tag or a container holds it"
    else
        printf '  candidate %-46s %8s\n' "$ref" "$(gb "$size")"
    fi
done <<EOF
$(docker images --format '{{.Repository}}:{{.Tag}}'$'\t''{{.Size}}' --no-trunc 2>/dev/null | awk -F'\t' '
  function bytes(n,   u,v) {
    u=n; sub(/^[0-9.]+/,"",u); gsub(/^ +| +$/,"",u); v=n+0
    if (u=="kB"||u=="KB") return v*1000; if (u=="MB") return v*1000000
    if (u=="GB") return v*1000000000; return v
  }
  # %.0f, not %d: mawk saturates %d at 2^31, so every image above 2.15 GB
  # reported as exactly 2.15 GB — and the total with it.
  { printf "%s\t%.0f\n", $1, bytes($2) }')
EOF

echo
if [ "$N" -eq 0 ]; then
    echo "  no test image left — nothing to do"
    exit 0
fi
printf '  %d image(s), %s\n' "$N" "$(gb "$TOTAL")"
if [ "$APPLY" -eq 1 ]; then
    printf '\nThe layers are freed, their build cache is NOT. Run now:\n  %s --apply dangling cache-dead\n' \
        "$(dirname "$0")/docker-reclaim.sh"
else
    printf '\nNothing was removed. Re-run with:  %s --apply\n' "$(basename "$0")"
fi
