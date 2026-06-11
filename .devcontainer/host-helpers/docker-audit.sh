#!/usr/bin/env bash
#
# .devcontainer/host-helpers/docker-audit.sh — audit the project's
# devcontainer images for size, layer sharing, and disk waste.
#
# Run on the HOST (not in the container — needs the local docker daemon).
# Auto-detects which images to inspect by parsing .devcontainer/Dockerfile*
# and .devcontainer/docker-compose.yml. Missing tags are reported and skipped.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/layers"

rev_lines() { awk '{a[NR]=$0} END{for(i=NR;i>=1;i--) print a[i]}'; }

human() {
	awk -v b="${1:-0}" 'BEGIN{
		split("B KB MB GB TB", u, " "); i=1
		while (b >= 1024 && i < 5) { b /= 1024; i++ }
		if (i == 1) printf "%d %s", b, u[i]
		else if (b < 10) printf "%.2f %s", b, u[i]
		else printf "%.1f %s", b, u[i]
	}'
}

if ! docker info >/dev/null 2>&1; then
	echo "Docker daemon unreachable. Check that Docker is running on the host." >&2
	exit 1
fi

# --- 1. Auto-detect expected tags ---

CLAUDE_VER="$(grep -oE 'CLAUDE_CODE_VERSION:-[0-9.]+' \
	"$REPO_ROOT/.devcontainer/initialize.sh" 2>/dev/null \
	| head -1 | sed 's/^[^0-9]*//' || true)"
CLAUDE_VER="${CLAUDE_VER:-latest}"

DC_PROJECT="$(grep -E '^DC_PROJECT=' "$REPO_ROOT/.devcontainer/.env" 2>/dev/null \
	| head -1 | sed 's/^DC_PROJECT=//' | tr -d '"' || true)"
DC_PROJECT="${DC_PROJECT:-devcontainer-tools}"
COMPOSE_NAME="${DC_PROJECT}-claude-code"

BASE_PARENT="$(grep -E '^FROM ' "$REPO_ROOT/.devcontainer/Dockerfile.base" 2>/dev/null \
	| head -1 | awk '{print $2}' || true)"
BRIDGE_PARENT="$(grep -E '^FROM ' \
	"$REPO_ROOT/.devcontainer/claude-bridge/Dockerfile" 2>/dev/null \
	| head -1 | awk '{print $2}' || true)"

EXPECTED=(
	"$BASE_PARENT"
	"claude-devcontainer-base:$CLAUDE_VER"
	"${COMPOSE_NAME}-app"
	"$BRIDGE_PARENT"
	"uniclaudeproxy:local"
)

LOCAL_TAGS=()
MISSING=()
for tag in "${EXPECTED[@]}"; do
	[ -z "$tag" ] && continue
	if docker image inspect "$tag" >/dev/null 2>&1; then
		LOCAL_TAGS+=("$tag")
	else
		MISSING+=("$tag")
	fi
done

if [ "${#MISSING[@]}" -gt 0 ]; then
	echo "Tags missing locally (skipped):"
	printf '  - %s\n' "${MISSING[@]}"
	echo
fi

if [ "${#LOCAL_TAGS[@]}" -eq 0 ]; then
	echo "No devcontainer image found locally." >&2
	exit 1
fi

# --- 2. Per-image data: total size + DIFF_IDs + per-layer sizes ---

: >"$WORK/sizes.tsv"
: >"$WORK/layersize.tsv"

i=0
for tag in "${LOCAL_TAGS[@]}"; do
	total="$(docker image inspect --format '{{.Size}}' "$tag")"
	docker image inspect --format '{{range .RootFS.Layers}}{{.}}{{"\n"}}{{end}}' "$tag" \
		| grep -v '^$' >"$WORK/layers/$i.txt"
	count="$(wc -l <"$WORK/layers/$i.txt" | tr -d ' ')"
	printf '%s\t%s\t%s\n' "$tag" "$total" "$count" >>"$WORK/sizes.tsv"

	# Align history (top-down, Size>0) reversed with RootFS.Layers (bottom-up).
	docker history --no-trunc --human=false --format '{{.Size}}' "$tag" \
		| awk '$1+0 > 0' \
		| rev_lines >"$WORK/sizes_only.txt"
	paste "$WORK/layers/$i.txt" "$WORK/sizes_only.txt" >>"$WORK/layersize.tsv"

	i=$((i+1))
done

# Dedup DIFF_ID -> bytes (a given DIFF_ID has a stable size).
sort -u -k1,1 -t$'\t' "$WORK/layersize.tsv" >"$WORK/layersize_uniq.tsv"

# --- 3. Section: Sizes ---

echo "== Sizes =="
printf '%-50s  %10s  %7s\n' "TAG" "SIZE" "LAYERS"
sort -k2,2 -nr -t$'\t' "$WORK/sizes.tsv" | while IFS=$'\t' read -r tag bytes layers; do
	printf '%-50s  %10s  %7s\n' "$tag" "$(human "$bytes")" "$layers"
done
total_naive="$(awk -F'\t' '{s+=$2} END{print s+0}' "$WORK/sizes.tsv")"
printf '%-50s  %10s\n' "TOTAL (naive — sum of images)" "$(human "$total_naive")"
echo

# --- 4. Section: Layer-sharing matrix ---

echo "== Layer sharing =="
echo "(common prefix / total in the smaller — a full prefix means"
echo " the FROM chain still shares perfectly.)"
echo

n="${#LOCAL_TAGS[@]}"
printf '%-50s' ""
for ((j=0; j<n; j++)); do printf '%8s' "[$j]"; done
echo

for ((i=0; i<n; i++)); do
	label="${LOCAL_TAGS[$i]}"
	[ "${#label}" -gt 46 ] && label="${label:0:46}"
	printf '[%d] %-46s' "$i" "$label"
	for ((j=0; j<n; j++)); do
		if [ "$i" -eq "$j" ]; then
			# ASCII '-' (1 byte = 1 display column). The em-dash '—'
			# (3 bytes, 1 col) breaks printf '%Ns' alignment because
			# the spec pads by bytes, not by display width.
			printf '%8s' "-"
			continue
		fi
		prefix=0
		while IFS= read -r line_i && IFS= read -r line_j <&3; do
			[ "$line_i" = "$line_j" ] || break
			prefix=$((prefix+1))
		done <"$WORK/layers/$i.txt" 3<"$WORK/layers/$j.txt"
		ci="$(wc -l <"$WORK/layers/$i.txt" | tr -d ' ')"
		cj="$(wc -l <"$WORK/layers/$j.txt" | tr -d ' ')"
		smaller="$ci"
		[ "$cj" -lt "$smaller" ] && smaller="$cj"
		printf '%8s' "$prefix/$smaller"
	done
	echo
done
echo

# --- 5. Section: Waste ---

echo "== Waste =="
subset_disk="$(awk -F'\t' '{s+=$2} END{print s+0}' "$WORK/layersize_uniq.tsv")"
saved=$((total_naive - subset_disk))
[ "$saved" -lt 0 ] && saved=0
if [ "$total_naive" -gt 0 ]; then
	pct="$(awk -v s="$saved" -v t="$total_naive" 'BEGIN{printf "%.1f", (s/t)*100}')"
else
	pct="0.0"
fi
printf '  Sum of images              : %s\n' "$(human "$total_naive")"
printf '  Real disk (subset)         : %s\n' "$(human "$subset_disk")"
printf '  Saved via sharing          : %s (%s %%)\n' "$(human "$saved")" "$pct"

# --- 6. Section: Cross-project audit of shared base ---
# Cross-project view of claude-devcontainer-base: for each local image,
# check if its first N layers (RootFS.Layers) match exactly the layers of
# a tagged base — exact match ⇒ derived image, sharing intact. An image
# matching no base is either non-derived or orphaned (original base
# deleted/overwritten). With per-project suffixed tags
# (`claude-devcontainer-base:VERSION-<project>`), there is no single
# "current base" anymore — each project has its own.

echo
echo "== Cross-project audit: claude-devcontainer-base =="

mkdir -p "$WORK/cross"

# Auto-discover all present base tags (version-sorted for stable display).
BASE_LIST=$(docker images --format '{{.Repository}}:{{.Tag}}' 2>/dev/null \
	| grep '^claude-devcontainer-base:' | sort -V -u)

if [ -z "$BASE_LIST" ]; then
	echo "No claude-devcontainer-base:* tag present locally."
	exit 0
fi

# Pre-compute display width of base name (after stripping the common
# prefix `claude-devcontainer-base:` — every base has it by construction).
base_w=$(echo "$BASE_LIST" | awk '
BEGIN{m=8}
{ s=$0; sub(/^claude-devcontainer-base:/,"",s); if(length(s)>m) m=length(s) }
END{print m}')

# Build each base's signature (layers + size).
# base_sigs.tsv : tag<TAB>layer_count<TAB>byte_size<TAB>sigfile
: >"$WORK/cross/base_sigs.tsv"
echo "Local bases (prefix 'claude-devcontainer-base:' stripped):"
echo "$BASE_LIST" | while IFS= read -r base; do
	[ -z "$base" ] && continue
	lf="$WORK/cross/$(echo "$base" | tr ':/' '__').sig"
	docker image inspect --format '{{range .RootFS.Layers}}{{.}}{{"\n"}}{{end}}' "$base" \
		| grep -v '^$' >"$lf"
	count=$(wc -l <"$lf" | tr -d ' ')
	size=$(docker image inspect --format '{{.Size}}' "$base")
	printf '%s\t%s\t%s\t%s\n' "$base" "$count" "$size" "$lf" >>"$WORK/cross/base_sigs.tsv"
	base_short="${base#claude-devcontainer-base:}"
	printf "  %-${base_w}s  %3s layers  %8s\n" \
		"$base_short" "$count" "$(human "$size")"
done
echo

# Scan all named images on the daemon (skip <none> and the bases themselves).
CANDIDATES=$(docker images --format '{{.Repository}}:{{.Tag}}' 2>/dev/null \
	| grep -v '<none>' \
	| grep -v '^claude-devcontainer-base:' \
	| sort -u)

# Pre-compute column widths after stripping common affixes
# (`-claude-code-app:latest` on the image side, `claude-devcontainer-base:`
# on the base side). No truncation: full names fit in ~96 chars total with
# current data, plenty for a 120+ char terminal.
#
# IMPORTANT: compute width ONLY on candidates that will actually be
# displayed (`-claude-code-app` in the name). Without that filter, unrelated
# images on the daemon (registries, other tools) with long names would
# bloat the PROJECT column unnecessarily.
img_w=$(echo "$CANDIDATES" | grep -- '-claude-code-app' | awk '
BEGIN{m=7}
{ s=$0; sub(/-claude-code-app:latest$/,"",s); if(length(s)>m) m=length(s) }
END{print m}')
# base_w already computed above (reused for the "Local bases" sub-table —
# lower bound 8 to accommodate "<orphan>").
[ "$base_w" -lt 8 ] && base_w=8
row_fmt="%-${img_w}s  %-${base_w}s  %8s  %12s\n"

echo "(image: '-claude-code-app:latest' stripped; base: 'claude-devcontainer-base:' stripped)"
printf "$row_fmt" "PROJECT" "BASE" "PREFIX" "DELTA"

# Counters + buckets in temp files: the `| while` runs in a subshell,
# variables there are lost on exit. Separate buckets so we can show
# matched ones first, orphans next.
echo 0 >"$WORK/cross/n_matched"
echo 0 >"$WORK/cross/n_orphan"
: >"$WORK/cross/matched.out"
: >"$WORK/cross/orphans.out"

# Second detection channel: the `-claude-code-app` suffix is the compose
# convention for project images (cf. COMPOSE_NAME above). An image that
# carries it but matches no base RootFS is very likely orphaned (original
# base deleted/overwritten) — display it anyway.
echo "$CANDIDATES" | while IFS= read -r img; do
	[ -z "$img" ] && continue
	imgf="$WORK/cross/img_$(echo "$img" | tr ':/' '__').layers"
	if ! docker image inspect --format '{{range .RootFS.Layers}}{{.}}{{"\n"}}{{end}}' "$img" 2>/dev/null \
		| grep -v '^$' >"$imgf"; then
		continue
	fi
	img_count=$(wc -l <"$imgf" | tr -d ' ')
	[ "$img_count" -eq 0 ] && continue

	matched_base=""
	matched_count=""
	matched_size=""
	while IFS=$'\t' read -r base bcount bsize bfile; do
		[ "$img_count" -lt "$bcount" ] && continue
		head -n "$bcount" "$imgf" >"$WORK/cross/_head.txt"
		if cmp -s "$WORK/cross/_head.txt" "$bfile"; then
			matched_base="$base"
			matched_count="$bcount"
			matched_size="$bsize"
			break
		fi
	done <"$WORK/cross/base_sigs.tsv"

	img_size=$(docker image inspect --format '{{.Size}}' "$img")

	if [ -n "$matched_base" ]; then
		delta=$((img_size - matched_size))
		[ "$delta" -lt 0 ] && delta=0

		n=$(($(cat "$WORK/cross/n_matched") + 1))
		echo "$n" >"$WORK/cross/n_matched"

		img_short="${img%-claude-code-app:latest}"
		matched_short="${matched_base#claude-devcontainer-base:}"
		printf "$row_fmt" \
			"$img_short" "$matched_short" \
			"${matched_count}/${img_count}" \
			"+$(human "$delta")" \
			>>"$WORK/cross/matched.out"
	elif echo "$img" | grep -q -- '-claude-code-app'; then
		# No base match — but project suffix: likely orphan.
		n=$(($(cat "$WORK/cross/n_orphan") + 1))
		echo "$n" >"$WORK/cross/n_orphan"
		img_short="${img%-claude-code-app:latest}"
		# ASCII '-' (vs em-dash '—') to align with %8s — printf
		# pads by bytes, and '—' is 3 bytes for 1 display column.
		printf "$row_fmt" \
			"$img_short" "<orphan>" "-/${img_count}" \
			"$(human "$img_size")" \
			>>"$WORK/cross/orphans.out"
	fi
done

# Display: matched first, orphans next.
cat "$WORK/cross/matched.out" "$WORK/cross/orphans.out"

n_matched=$(cat "$WORK/cross/n_matched" 2>/dev/null || echo 0)
n_orphan=$(cat "$WORK/cross/n_orphan" 2>/dev/null || echo 0)
total=$((n_matched + n_orphan))

echo
if [ "$total" -eq 0 ]; then
	echo "No derived image detected among local tags."
else
	echo "($n_matched derived match(es), $n_orphan orphan(s))"
fi
echo
echo "Images not listed either don't derive from any present"
echo "claude-devcontainer-base:*, or were built on a deleted/overwritten"
echo "base — rebuild advised to recover sharing."
