#!/usr/bin/env bash
# Unit tests for compile-policy.py --split-local mode.
# Standalone — only requires python3 + grep. No jq needed.
# Usage: ./split-local.sh

set -uo pipefail

THIS_DIR="$(cd "$(dirname "$(readlink -f "$0" 2>/dev/null || realpath "$0")")" && pwd)"
COMPILE="$THIS_DIR/../compile-policy.py"

if [ ! -f "$COMPILE" ]; then
  echo "❌ compile-policy.py not found at $COMPILE" >&2
  exit 1
fi

PASS=0
FAIL=0

expect_eq() {
  local desc="$1" actual="$2" expected="$3"
  if [ "$actual" = "$expected" ]; then
    echo "  ✔ $desc"
    PASS=$((PASS+1))
  else
    echo "  ❌ $desc"
    echo "      expected: $expected"
    echo "      actual:   $actual"
    FAIL=$((FAIL+1))
  fi
}

# compile_split <cfg_dir> <base_out> <local_out>
# Runs compile-policy.py --split-local against a temp config dir.
compile_split() {
  local cfg_dir="$1" base_out="$2" local_out="$3"
  local out_pol; out_pol=$(mktemp)
  python3 "$COMPILE" --config-dir "$cfg_dir" \
    --split-local \
    --out-dnsmasq-base  "$base_out" \
    --out-dnsmasq-local "$local_out" \
    --out-policy        "$out_pol" >/dev/null 2>&1
  local rc=$?
  rm -f "$out_pol"
  return $rc
}

# has_host <conf_file> <host> → prints "yes" | "no"
has_host() {
  if grep -qE "^server=/$2/" "$1" 2>/dev/null; then
    echo "yes"
  else
    echo "no"
  fi
}

# ipset_of <conf_file> <host> → prints the ipset name for a host, or "none"
ipset_of() {
  local line
  line=$(grep -E "^ipset=/$2/" "$1" 2>/dev/null | head -1)
  if [ -z "$line" ]; then
    echo "none"
  else
    echo "${line##*/}"
  fi
}

# --- A. Basic split — baseline vs local partition ---
echo "▌ A. Baseline vs local partition"

cfg=$(mktemp -d)
mkdir -p "$cfg/domains.d" "$cfg/policy.d" "$cfg/policy.local.d"
echo 'api.baseline.com' > "$cfg/domains.txt"
echo 'newhost.local.com' > "$cfg/domains.local.txt"
base_out=$(mktemp); local_out=$(mktemp)
compile_split "$cfg" "$base_out" "$local_out"

expect_eq "A1. baseline host in base file"   "$(has_host "$base_out"  api.baseline.com)"  "yes"
expect_eq "A2. baseline host NOT in local"   "$(has_host "$local_out" api.baseline.com)"  "no"
expect_eq "A3. local host in local file"     "$(has_host "$local_out" newhost.local.com)" "yes"
expect_eq "A4. local host NOT in base"       "$(has_host "$base_out"  newhost.local.com)" "no"
expect_eq "A5. base uses base ipset name"    "$(ipset_of "$base_out"  api.baseline.com)"  "allowed-domains-base"
expect_eq "A6. local uses local ipset name"  "$(ipset_of "$local_out" newhost.local.com)" "allowed-domains-local"

rm -rf "$cfg" "$base_out" "$local_out"

# --- B. domains.d/ counts as base ---
echo "▌ B. domains.d/ counts as baseline"

cfg=$(mktemp -d)
mkdir -p "$cfg/domains.d" "$cfg/policy.d" "$cfg/policy.local.d"
echo 'core.baseline.com' > "$cfg/domains.txt"
echo 'npm.eco.com' > "$cfg/domains.d/npm.txt"
echo 'newhost.local.com' > "$cfg/domains.local.txt"
base_out=$(mktemp); local_out=$(mktemp)
compile_split "$cfg" "$base_out" "$local_out"

expect_eq "B1. domains.d host in base file" "$(has_host "$base_out"  npm.eco.com)"       "yes"
expect_eq "B2. domains.d host NOT in local" "$(has_host "$local_out" npm.eco.com)"       "no"
expect_eq "B3. domains.txt still in base"   "$(has_host "$base_out"  core.baseline.com)" "yes"
expect_eq "B4. local host in local file"    "$(has_host "$local_out" newhost.local.com)" "yes"

rm -rf "$cfg" "$base_out" "$local_out"

# --- C. !disable removes host from base ---
echo "▌ C. !disable of baseline host"

cfg=$(mktemp -d)
mkdir -p "$cfg/domains.d" "$cfg/policy.d" "$cfg/policy.local.d"
echo 'stay.baseline.com' > "$cfg/domains.txt"
echo 'kill.baseline.com' >> "$cfg/domains.txt"
echo '!disable kill.baseline.com' > "$cfg/domains.local.txt"
base_out=$(mktemp); local_out=$(mktemp)
compile_split "$cfg" "$base_out" "$local_out"

expect_eq "C1. disabled host absent from base"  "$(has_host "$base_out"  kill.baseline.com)" "no"
expect_eq "C2. disabled host absent from local" "$(has_host "$local_out" kill.baseline.com)" "no"
expect_eq "C3. other baseline still present"    "$(has_host "$base_out"  stay.baseline.com)" "yes"

rm -rf "$cfg" "$base_out" "$local_out"

# --- D. Redefine baseline via local — stays in base ---
echo "▌ D. Redefine baseline via local layer"

cfg=$(mktemp -d)
mkdir -p "$cfg/domains.d" "$cfg/policy.d" "$cfg/policy.local.d"
echo '[GET] shared.baseline.com' > "$cfg/domains.txt"
echo '[GET,POST] shared.baseline.com' > "$cfg/domains.local.txt"
base_out=$(mktemp); local_out=$(mktemp)
compile_split "$cfg" "$base_out" "$local_out"

expect_eq "D1. redefined host stays in base"    "$(has_host "$base_out"  shared.baseline.com)" "yes"
expect_eq "D2. redefined host NOT in local"     "$(has_host "$local_out" shared.baseline.com)" "no"
expect_eq "D3. redefined keeps base ipset"      "$(ipset_of "$base_out"  shared.baseline.com)" "allowed-domains-base"

rm -rf "$cfg" "$base_out" "$local_out"

# --- E. Empty local file → empty local ipset file ---
echo "▌ E. Empty local layer → empty local file"

cfg=$(mktemp -d)
mkdir -p "$cfg/domains.d" "$cfg/policy.d" "$cfg/policy.local.d"
echo 'only.baseline.com' > "$cfg/domains.txt"
touch "$cfg/domains.local.txt"
base_out=$(mktemp); local_out=$(mktemp)
compile_split "$cfg" "$base_out" "$local_out"

expect_eq "E1. baseline in base"                "$(has_host "$base_out"  only.baseline.com)"  "yes"
local_hosts_count=$(grep -c '^server=/' "$local_out" 2>/dev/null; true)
expect_eq "E2. local file has 0 hosts"          "$local_hosts_count"                          "0"

rm -rf "$cfg" "$base_out" "$local_out"

# --- F. Regression — legacy --out-dnsmasq path unchanged ---
echo "▌ F. Regression: legacy --out-dnsmasq path"

cfg=$(mktemp -d)
mkdir -p "$cfg/domains.d" "$cfg/policy.d" "$cfg/policy.local.d"
echo 'legacy.host.com' > "$cfg/domains.txt"
touch "$cfg/domains.local.txt"

legacy_out=$(mktemp); pol_out=$(mktemp)
python3 "$COMPILE" --config-dir "$cfg" \
  --out-dnsmasq "$legacy_out" \
  --out-policy  "$pol_out" >/dev/null 2>&1

expect_eq "F1. legacy mode: host present"      "$(has_host "$legacy_out" legacy.host.com)" "yes"
expect_eq "F2. legacy mode: ipset unchanged"   "$(ipset_of "$legacy_out" legacy.host.com)" "allowed-domains"

rm -rf "$cfg" "$legacy_out" "$pol_out"

# --- G. policy.local.d host goes to local ipset ---
echo "▌ G. policy.local.d ships hosts to local partition"

cfg=$(mktemp -d)
mkdir -p "$cfg/domains.d" "$cfg/policy.d" "$cfg/policy.local.d"
echo 'baseline.only.com' > "$cfg/domains.txt"
touch "$cfg/domains.local.txt"

cat > "$cfg/policy.local.d/extra.local.com.yaml" <<EOF
allowed_methods: [GET]
paths: ["/*"]
EOF
echo 'extra.local.com' >> "$cfg/domains.local.txt"

base_out=$(mktemp); local_out=$(mktemp)
compile_split "$cfg" "$base_out" "$local_out"

expect_eq "G1. policy.local.d host in local"   "$(has_host "$local_out" extra.local.com)"  "yes"
expect_eq "G2. policy.local.d host NOT in base" "$(has_host "$base_out" extra.local.com)"  "no"

rm -rf "$cfg" "$base_out" "$local_out"

echo
echo "═══════════════════════════════════════════════════════"
echo "  Tests : $((PASS+FAIL)) | ✔ $PASS | ❌ $FAIL"
echo "═══════════════════════════════════════════════════════"

[ "$FAIL" -eq 0 ]
