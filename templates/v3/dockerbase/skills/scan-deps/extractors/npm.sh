#!/usr/bin/env bash
# extractors/npm.sh — deterministic npm allowlist extractor.
#
# Walks every package.json under <root> (depth-bumped, excluding node_modules,
# vendor, .git, research-bundles). For each manifest :
#   - direct deps + devDeps     (always)
#   - package-lock.json packages (if present — full transitive list)
#   - node_modules/* walk        (if present — repo.url + binary.host + postinstall)
#
# Writes :
#   .devcontainer/firewall/domains.d/npm.txt              (per-dep + repos + binaries)
#   .devcontainer/firewall/domains.d/ecosystem-docs.txt   (curated docs for detected libs)
#
# Offline only. Atomic write (.tmp + mv).
# Usage : bash extractors/npm.sh <search-root>

# Note: not using `set -u` because we manipulate many associative arrays
# that may stay empty (no repos found, no postinstall hooks, etc.) — bash
# treats `${#arr[@]}` on a declared-but-empty assoc array as "unbound".

SEARCH_ROOT="${1:?usage: npm.sh <search-root>}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/lib"

# shellcheck source=lib/common.sh
. "$LIB_DIR/common.sh"

DOCS_MAPPING="$LIB_DIR/npm-docs-mapping.txt"
DOMAINS_D="/workspace/.devcontainer/firewall/domains.d"
NPM_OUT="$DOMAINS_D/npm.txt"
DOCS_OUT="$DOMAINS_D/ecosystem-docs.txt"

mkdir -p "$DOMAINS_D"

# ─── 1. Find all package.json under root ─────────────────────────
MANIFESTS=$(find_manifests "$SEARCH_ROOT" package.json)
if [ -z "$MANIFESTS" ]; then
  echo "extract-auto npm: no package.json found under $SEARCH_ROOT — skipping"
  exit 0
fi

# ─── 2. Aggregate state ──────────────────────────────────────────
declare -A DEPS              # dep_name → 1 (set semantics)
declare -A REPOS             # owner/repo → 1
declare -A BINARY_HOSTS      # host → "owner/repo"|"_path_"
declare -A POSTINSTALL_URLS  # url → script-name
declare -A DOC_HOSTS         # host → comment

# Always-on docs for npm projects
DOC_HOSTS["nodejs.org"]="Node.js runtime documentation"
DOC_HOSTS["developer.mozilla.org"]="MDN Web Docs"

# ─── 3. Process each manifest ────────────────────────────────────
while IFS= read -r manifest; do
  [ -z "$manifest" ] && continue
  dir=$(dirname "$manifest")
  lock="$dir/package-lock.json"
  nm="$dir/node_modules"

  # 3a. Direct + dev deps from package.json
  while IFS= read -r d; do
    [ -z "$d" ] && continue
    DEPS["$d"]=1
  done < <(jq -r '(.dependencies // {}) + (.devDependencies // {}) | keys[]?' "$manifest" 2>/dev/null)

  # 3b. Transitive deps from package-lock.json `.packages` keys
  if [ -f "$lock" ]; then
    while IFS= read -r d; do
      [ -z "$d" ] && continue
      DEPS["$d"]=1
    done < <(jq -r '.packages // {} | keys[]? | select(startswith("node_modules/")) | split("/node_modules/") | .[-1]' "$lock" 2>/dev/null)

    # 3b'. Git-sourced deps from .packages[*].resolved (when a dep is installed
    # FROM git rather than the registry — npm rewrites e.g.
    # "git+ssh://git@github.com/owner/repo.git" → resolved). We need allowlist
    # entries on the host serving the tarball/refs so future `npm install`
    # works.
    while IFS= read -r url; do
      [ -z "$url" ] && continue
      # Strip scheme prefix + suffixes
      cleaned=$(echo "$url" | sed -E 's|^git\+||; s|^ssh://git@|https://|; s|^git://|https://|; s|#.*$||; s|\.git$||')
      if [[ "$cleaned" =~ ^https?://([^/]+)/([^/]+)/([^/?#]+) ]]; then
        rhost="${BASH_REMATCH[1]}"
        rowner="${BASH_REMATCH[2]}"
        rrepo="${BASH_REMATCH[3]}"
        if [ "$rhost" = "github.com" ] || [ "$rhost" = "gitlab.com" ] || [ "$rhost" = "bitbucket.org" ]; then
          REPOS["${rhost}|${rowner}/${rrepo}"]=1
        fi
      fi
    done < <(jq -r '.packages // {} | to_entries[]? | .value.resolved // empty | select(startswith("git+") or startswith("git://") or startswith("ssh://"))' "$lock" 2>/dev/null)
  fi

  # 3c. Walk node_modules/<dep>/package.json (top-level + scoped + nested)
  if [ -d "$nm" ]; then
    while IFS= read -r pkg; do
      [ -f "$pkg" ] || continue
      # Skip the top-level package.json (already processed via $manifest)
      [ "$pkg" = "$manifest" ] && continue

      # Dep name from path (last "node_modules/<here>")
      dep=$(echo "$pkg" | sed -E "s|.*/node_modules/||; s|/package.json$||")
      DEPS["$dep"]=1

      # .repository.url → owner/repo on github.com (or other hosts)
      repo=$(jq -r '.repository.url // .repository // empty' "$pkg" 2>/dev/null \
             | sed -E 's|^git\+||; s|^git://|https://|; s|\.git$||')
      if [[ "$repo" =~ ^https?://([^/]+)/([^/]+)/([^/?#]+) ]]; then
        rhost="${BASH_REMATCH[1]}"
        rowner="${BASH_REMATCH[2]}"
        rrepo="${BASH_REMATCH[3]}"
        if [ "$rhost" = "github.com" ] || [ "$rhost" = "gitlab.com" ] || [ "$rhost" = "bitbucket.org" ]; then
          REPOS["${rhost}|${rowner}/${rrepo}"]=1
        fi
      fi

      # node-pre-gyp postinstall .binary.host + .binary.remote_path
      bhost=$(jq -r '.binary.host // empty' "$pkg" 2>/dev/null \
              | sed -E 's|https?://||; s|/.*||')
      if [ -n "$bhost" ]; then
        bpath=$(jq -r '.binary.remote_path // empty' "$pkg" 2>/dev/null \
                | tr -d '\n')
        BINARY_HOSTS["$bhost"]="${bpath:-/*}"
      fi

      # .scripts.{preinstall,install,postinstall} URL extraction (best-effort)
      for hook in preinstall install postinstall; do
        script=$(jq -r ".scripts.${hook} // empty" "$pkg" 2>/dev/null)
        [ -z "$script" ] && continue
        # Extract https?:// URLs from the script string
        while IFS= read -r url; do
          [ -z "$url" ] && continue
          POSTINSTALL_URLS["$url"]="$dep:$hook"
        done < <(echo "$script" | grep -oE 'https?://[A-Za-z0-9._/-]+' || true)
      done
    done < <(find "$nm" -maxdepth 4 -name package.json -type f 2>/dev/null)
  fi
done <<< "$MANIFESTS"

# ─── 4. Lookup doc sites for detected deps ───────────────────────
for dep in "${!DEPS[@]}"; do
  doc=$(lookup_doc_site "$dep" "$DOCS_MAPPING")
  [ -z "$doc" ] && continue
  DOC_HOSTS["$doc"]="detected: $dep"
done

# ─── 5. Emit domains.d/npm.txt ───────────────────────────────────
{
  emit_header npm "extract-auto-dependencies (extractors/npm.sh)"

  echo "# --- npm registry (per-dep, $(echo "${!DEPS[@]}" | wc -w | tr -d ' ') packages) ---"
  emit_format3_block "GET,HEAD" "registry.npmjs.org"
  # Sort dep names for stable diff
  printf '%s\n' "${!DEPS[@]}" | sort -u | while IFS= read -r dep; do
    emit_format3_path "/${dep}*"
  done

  if [ ${#REPOS[@]} -gt 0 ]; then
    echo
    echo "# --- Git repos (from node_modules/.repository.url + package-lock git+ sources) ---"
    # Group by host
    declare -A REPOS_BY_HOST
    for key in "${!REPOS[@]}"; do
      host="${key%%|*}"
      pathseg="${key#*|}"
      REPOS_BY_HOST["$host"]="${REPOS_BY_HOST[$host]:-}${pathseg}|"
    done
    for host in "${!REPOS_BY_HOST[@]}"; do
      emit_format3_block "GET" "$host"
      printf '%s\n' "${REPOS_BY_HOST[$host]}" | tr '|' '\n' | sort -u | while IFS= read -r p; do
        [ -z "$p" ] && continue
        emit_format3_path "/${p}*"
      done
    done
  fi

  if [ ${#BINARY_HOSTS[@]} -gt 0 ]; then
    echo
    echo "# --- Postinstall (node-pre-gyp .binary.host + .binary.remote_path) ---"
    for host in "${!BINARY_HOSTS[@]}"; do
      bpath="${BINARY_HOSTS[$host]}"
      emit_format3_block "GET" "$host"
      # Trim trailing wildcards repeatedly to get a clean prefix
      bpath_clean=$(echo "$bpath" | sed -E 's|/\*$||; s|/\{[^}]*\}|/*|g')
      [ -z "$bpath_clean" ] && bpath_clean="/"
      emit_format3_path "${bpath_clean}*"
    done
  fi

  if [ ${#POSTINSTALL_URLS[@]} -gt 0 ]; then
    echo
    echo "# --- .scripts.{pre,install,post}install detected URLs (best-effort, manual review recommended) ---"
    declare -A POST_HOSTS
    for url in "${!POSTINSTALL_URLS[@]}"; do
      host=$(echo "$url" | sed -E 's|https?://||; s|/.*||')
      POST_HOSTS["$host"]="${POSTINSTALL_URLS[$url]}"
    done
    for host in "${!POST_HOSTS[@]}"; do
      printf '%-40s # postinstall — %s\n' "$host" "${POST_HOSTS[$host]}"
    done
  fi
} | atomic_write "$NPM_OUT"

# ─── 6. Emit domains.d/ecosystem-docs.txt ────────────────────────
{
  emit_header ecosystem-docs "extract-auto-dependencies (extractors/npm.sh)"
  echo "# Curated documentation sites for detected libraries."
  echo "# Add new entries to extractors/lib/npm-docs-mapping.txt."
  echo
  for host in $(printf '%s\n' "${!DOC_HOSTS[@]}" | sort -u); do
    note="${DOC_HOSTS[$host]}"
    printf '%-40s # %s\n' "$host" "$note"
  done
} | atomic_write "$DOCS_OUT"

# ─── 7. Summary to stdout ────────────────────────────────────────
echo "✓ extract-auto npm:"
echo "    $(echo "${!DEPS[@]}" | wc -w | tr -d ' ') deps (direct + transitive)"
echo "    ${#REPOS[@]} GitHub repos (from node_modules/.repository.url)"
echo "    ${#BINARY_HOSTS[@]} postinstall binary hosts"
echo "    ${#POSTINSTALL_URLS[@]} URLs from .scripts.*install"
echo "    ${#DOC_HOSTS[@]} doc sites (curated)"
echo "    → $NPM_OUT"
echo "    → $DOCS_OUT"
