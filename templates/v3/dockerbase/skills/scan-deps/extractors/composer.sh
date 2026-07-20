#!/usr/bin/env bash
# extractors/composer.sh — deterministic composer.lock allowlist extractor.
#
# Walks every composer.lock under <root>, parallel to npm.sh. For each
# entry in .packages[] + .packages-dev[] :
#   1. GitHub dist URL (api.github.com /repos/V/R/zipball/SHA)
#        → emit /repos/V/R/* on api.github.com (GET,HEAD)
#   2. Online HEAD on dist URL — if 302 to /repositories/<id>/zipball/SHA,
#        emit /repositories/<id>/* (skipped under --offline; composer then
#        falls back to source-clone via §4)
#   3. Codeload CDN redirect target → codeload.github.com /V/R/* (GET)
#   4. GitHub source-clone fallback (.source.type=git on github.com)
#        → github.com /V/R.git/* (GET,POST — info/refs + git-upload-pack)
#   5. Packagist metadata → repo.packagist.org /p2/<vendor>/* + /packages.json
#   6. Legacy fallback → packagist.org /packages.json (composer 1 compat)
#   7. Non-github / non-packagist dist URLs → emitted as commented entries
#      (manual review, not silently dropped)
#
# Granularity: repo-level on github, vendor-level on packagist. Survives
# `composer update` without re-running scan-deps unless a NEW vendor or
# repo enters the lock.
#
# Writes : .devcontainer/firewall/domains.d/composer.txt (atomic .tmp+mv).
#
# Online by default — step 2 issues HEAD requests to api.github.com.
# Pass --offline to skip step 2 (deterministic, no network); composer
# falls back to source-clone (§4) for renamed/transferred repos.
#
# Usage : bash extractors/composer.sh <search-root> [--offline]
#
# Note: not using `set -u` for the same reason as npm.sh — declared-but-empty
# associative arrays are flagged as unbound, which we don't want.

SEARCH_ROOT="${1:?usage: composer.sh <search-root> [--offline]}"
shift || true

OFFLINE=0
while [ $# -gt 0 ]; do
  case "$1" in
    --offline) OFFLINE=1; shift ;;
    *) echo "composer.sh: unknown option: $1" >&2; exit 2 ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/lib"

# shellcheck source=lib/common.sh
. "$LIB_DIR/common.sh"

DOMAINS_D="/workspace/.devcontainer/firewall/domains.d"
COMPOSER_OUT="$DOMAINS_D/composer.txt"

mkdir -p "$DOMAINS_D"

# ─── 1. Find all composer.lock under root ────────────────────────
MANIFESTS=$(find_manifests "$SEARCH_ROOT" composer.lock)
if [ -z "$MANIFESTS" ]; then
  echo "extract-auto composer: no composer.lock found under $SEARCH_ROOT — skipping"
  exit 0
fi

# ─── 2. Aggregate state ──────────────────────────────────────────
declare -A GH_REPOS         # "owner/repo" → 1   (from .dist.url, github zipball)
declare -A GH_GIT_REPOS     # "owner/repo" → 1   (from .source.url, github .git)
declare -A VENDORS          # vendor → 1         (from .name, lowercase)
declare -A NUMERIC_IDS      # id → "owner/repo"  (from online HEAD redirect)
declare -A SKIPPED          # package_name → dist_url  (non-github dist)

# ─── 3. Process each composer.lock ───────────────────────────────
while IFS= read -r lock; do
  [ -z "$lock" ] && continue

  while IFS=$'\t' read -r name disturl sourcetype sourceurl; do
    [ -z "$name" ] && continue

    # Vendor extraction (.name = "vendor/package", lowercase)
    vendor=$(echo "$name" | cut -d/ -f1 | tr 'A-Z' 'a-z')
    [ -n "$vendor" ] && VENDORS["$vendor"]=1

    # GitHub dist URL ?
    if [[ "$disturl" =~ ^https://api\.github\.com/repos/([^/]+)/([^/]+)/zipball/[a-f0-9]{40}$ ]]; then
      gh_owner="${BASH_REMATCH[1]}"
      gh_repo="${BASH_REMATCH[2]}"
      GH_REPOS["${gh_owner}/${gh_repo}"]=1

      # Numeric-ID redirect resolution (online HEAD).
      # api.github.com sometimes 302-redirects zipball requests to
      # /repositories/<numeric-id>/zipball/<sha> for renamed/transferred
      # repos. The ID isn't in composer.lock, so we resolve it once via HEAD.
      # If the firewall blocks the HEAD (first-run bootstrap before the new
      # composer.txt is reloaded), curl returns empty redirect_url and we
      # skip silently — composer will fall back to source-clone (§4).
      if [ $OFFLINE -eq 0 ]; then
        location=$(curl -sI -o /dev/null -w '%{redirect_url}' \
                        --max-time 5 --connect-timeout 3 \
                        "$disturl" 2>/dev/null || echo "")
        if [[ "$location" =~ /repositories/([0-9]+)/zipball/ ]]; then
          NUMERIC_IDS["${BASH_REMATCH[1]}"]="${gh_owner}/${gh_repo}"
        fi
      fi
    elif [ -n "$disturl" ] && [ "$disturl" != "null" ]; then
      SKIPPED["$name"]="$disturl"
    fi

    # Source git URL on github.com ? (.source.type=git, .source.url=https://github.com/V/R.git)
    if [ "$sourcetype" = "git" ] && [[ "$sourceurl" =~ ^https://github\.com/([^/]+)/(.+)\.git$ ]]; then
      sgh_owner="${BASH_REMATCH[1]}"
      sgh_repo="${BASH_REMATCH[2]}"
      GH_GIT_REPOS["${sgh_owner}/${sgh_repo}"]=1
    fi
  done < <(jq -r '
    ((.packages // []) + (.["packages-dev"] // []))
    | .[]
    | [.name, (.dist.url // ""), (.source.type // ""), (.source.url // "")]
    | @tsv
  ' "$lock" 2>/dev/null)
done <<< "$MANIFESTS"

# ─── 4. Emit composer.txt ────────────────────────────────────────
{
  emit_header composer "extract-auto-dependencies (extractors/composer.sh)"

  # 4a. api.github.com — /repos/V/R/* + /repositories/<id>/*
  if [ ${#GH_REPOS[@]} -gt 0 ]; then
    n_repos=$(printf '%s\n' "${!GH_REPOS[@]}" | wc -l | tr -d ' ')
    echo "# --- GitHub API (composer dist tarballs, ${n_repos} repos) ---"
    emit_format3_block "GET,HEAD" "api.github.com"
    printf '%s\n' "${!GH_REPOS[@]}" | sort -u | while IFS= read -r r; do
      emit_format3_path "/repos/${r}/*"
    done
    if [ ${#NUMERIC_IDS[@]} -gt 0 ]; then
      printf '%s\n' "${!NUMERIC_IDS[@]}" | sort -un | while IFS= read -r id; do
        emit_format3_path "/repositories/${id}/*" "→ ${NUMERIC_IDS[$id]} (renamed/transferred)"
      done
    fi
  fi

  # 4b. codeload.github.com — /V/R/* (api → codeload 302 target)
  if [ ${#GH_REPOS[@]} -gt 0 ]; then
    echo
    echo "# --- GitHub codeload CDN (api → codeload 302 redirect target) ---"
    emit_format3_block "GET" "codeload.github.com"
    printf '%s\n' "${!GH_REPOS[@]}" | sort -u | while IFS= read -r r; do
      emit_format3_path "/${r}/*"
    done
  fi

  # 4c. github.com — /V/R.git/* (source-clone + VCS-only repos + offline fallback)
  if [ ${#GH_GIT_REPOS[@]} -gt 0 ]; then
    echo
    echo "# --- GitHub source-clone (composer install --prefer-source + VCS-only repos) ---"
    emit_format3_block "GET,POST" "github.com"
    printf '%s\n' "${!GH_GIT_REPOS[@]}" | sort -u | while IFS= read -r r; do
      emit_format3_path "/${r}.git/*"
    done
  fi

  # 4d. repo.packagist.org — /p2/V/* per vendor + /packages.json
  if [ ${#VENDORS[@]} -gt 0 ]; then
    n_vendors=$(printf '%s\n' "${!VENDORS[@]}" | wc -l | tr -d ' ')
    echo
    echo "# --- Packagist metadata (${n_vendors} vendors) ---"
    emit_format3_block "GET" "repo.packagist.org"
    emit_format3_path "/packages.json" "root index"
    printf '%s\n' "${!VENDORS[@]}" | sort -u | while IFS= read -r v; do
      emit_format3_path "/p2/${v}/*"
    done
  fi

  # 4e. packagist.org — legacy fallback (composer 1 compat)
  echo
  echo "# --- Packagist legacy (composer 1 compat) ---"
  emit_format3_block "GET" "packagist.org"
  emit_format3_path "/packages.json" "composer 1 root index"

  # 4f. Skipped entries (non-github dist URLs — manual review)
  if [ ${#SKIPPED[@]} -gt 0 ]; then
    echo
    echo "# --- Skipped (non-github dist URLs — review manually, add to domains.local.txt if trusted) ---"
    while IFS= read -r name; do
      [ -z "$name" ] && continue
      printf '# %s → %s\n' "$name" "${SKIPPED[$name]}"
    done < <(printf '%s\n' "${!SKIPPED[@]}" | sort -u)
  fi
} | atomic_write "$COMPOSER_OUT"

# ─── 5. Summary to stdout ────────────────────────────────────────
echo "✓ extract-auto composer:"
echo "    ${#GH_REPOS[@]} GitHub dist repos"
[ ${#NUMERIC_IDS[@]} -gt 0 ] && echo "    ${#NUMERIC_IDS[@]} numeric-ID redirects (resolved via online HEAD)"
[ ${#GH_GIT_REPOS[@]} -gt 0 ] && echo "    ${#GH_GIT_REPOS[@]} GitHub source-clone repos"
echo "    ${#VENDORS[@]} packagist vendors"
[ ${#SKIPPED[@]} -gt 0 ] && echo "    ${#SKIPPED[@]} skipped (non-github dist — see commented entries)"
echo "    → $COMPOSER_OUT"
[ $OFFLINE -eq 1 ] && echo "    (--offline: numeric-ID resolution skipped)"
