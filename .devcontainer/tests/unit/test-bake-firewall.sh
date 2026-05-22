#!/usr/bin/env bash
# unit/test-bake-firewall.sh — static repo invariants for session 1.
#
# Runs from host OR container, no rebuild required. Validates that the
# bake-only changes are correctly applied at the file level :
#   - new baked files present (default-mode, direct-tcp-allow.txt, .keep)
#   - bind mount removed from docker-compose
#   - recursive COPY in Dockerfile + Dockerfile.php
#   - init-firewall.sh / test-firewall.sh read from baked paths
#   - .gitignore correctly structured
#   - install.sh declares migration + copies new files
#   - migration logic produces the right output on a tmpdir

source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
ROOT="$(repo_root)"

test_shell_scripts_parse() {
  local files=(
    "templates/v2/init-firewall.sh"
    "templates/v2/test-firewall.sh"
    "templates/v2/post-start.sh"
    "templates/v2/on-create.sh"
    "templates/v2/initialize.sh"
    "templates/v2/firewall-mode.sh"
    "templates/v2/shell-init.sh"
    "templates/v2/host-helpers/claude-switch"
    "install.sh"
  )
  for f in "${files[@]}"; do
    assert_file_exists "$ROOT/$f" "$f exists"
    assert_true bash -n "$ROOT/$f" -- "$f syntax OK"
  done
}

test_baked_files_present() {
  local paths=(
    "templates/v2/firewall/default-mode"
    "templates/v2/firewall/direct-tcp-allow.txt"
    "templates/v2/firewall/policy.local.d/.keep"
    ".devcontainer/firewall/default-mode"
    ".devcontainer/firewall/direct-tcp-allow.txt"
    ".devcontainer/firewall/policy.local.d/.keep"
  )
  for p in "${paths[@]}"; do
    assert_file_exists "$ROOT/$p" "$p"
  done
  assert_eq_file_content "$ROOT/templates/v2/firewall/default-mode" "strict" "template default-mode = strict"
  assert_eq_file_content "$ROOT/.devcontainer/firewall/default-mode" "strict" "dogfood default-mode = strict"
}

test_bind_mount_removed() {
  for f in "templates/v2/docker-compose.yml" ".devcontainer/docker-compose.yml"; do
    assert_false grep -qE "^\s*-\s*\./firewall:/etc/devcontainer-firewall" "$ROOT/$f" -- \
      "$f has no firewall bind mount"
  done
}

test_recursive_copy_in_dockerfiles() {
  for f in "templates/v2/Dockerfile" "templates/v2/Dockerfile.php" ".devcontainer/Dockerfile"; do
    assert_match "$ROOT/$f" "^COPY[[:space:]]+firewall/[[:space:]]+/etc/devcontainer-firewall/" \
      "$f recursive COPY firewall/"
  done
}

test_ranges_d_removed() {
  assert_false test -d "$ROOT/templates/v2/firewall/ranges.d" -- "templates ranges.d removed"
}

test_init_firewall_reads_baked() {
  local f="$ROOT/templates/v2/init-firewall.sh"
  assert_contains "$f" "/etc/devcontainer-firewall/default-mode" \
    "init-firewall.sh reads default-mode baked"
  assert_contains "$f" "/etc/devcontainer-firewall/direct-tcp-allow.txt" \
    "init-firewall.sh reads direct-tcp-allow.txt baked"
}

test_test_firewall_reads_baked() {
  local f="$ROOT/templates/v2/test-firewall.sh"
  assert_contains "$f" "/etc/devcontainer-firewall/default-mode" \
    "test-firewall.sh reads default-mode baked"
  assert_contains "$f" "/etc/devcontainer-firewall/direct-tcp-allow.txt" \
    "test-firewall.sh reads direct-tcp-allow.txt baked"
}

test_gitignore_grouped() {
  local f="$ROOT/templates/v2/.gitignore"
  assert_contains "$f" "!firewall/policy.local.d/.keep" \
    ".gitignore tracks !firewall/policy.local.d/.keep"
  # Skill scratch dirs : all `xxx/*` must appear BEFORE any `!xxx/.keep`.
  if awk '
    /^!pending\/\.keep/ { neg=1 }
    /^pending\/\*$/      { if (neg) bad=1 }
    END { exit bad?1:0 }
  ' "$f"; then
    _ok ".gitignore : skill negations grouped after ignores"
  else
    _nok ".gitignore : interleaved !negation with ignore"
  fi
}

test_install_sh_has_migration() {
  local f="$ROOT/install.sh"
  assert_contains "$f" "migrate_legacy_firewall" "install.sh declares migrate_legacy_firewall"
  assert_contains "$f" "copy_dir firewall/policy.local.d" "install.sh copies policy.local.d/"
  assert_contains "$f" "default-mode direct-tcp-allow.txt" "install.sh copies new baked files"
}

test_migration_dry_run() {
  local TMP; TMP=$(mktemp -d)
  trap "rm -rf '$TMP'" RETURN
  mkdir -p "$TMP/firewall"
  echo "basic" > "$TMP/.configured-firewall-mode"
  cat > "$TMP/.env" <<EOF
TZ=Europe/Paris
CLAUDE_CODE_FIREWALL_ALLOWED=claude-bridge:9223,host:11434
CLAUDE_CODE_VERSION=2.1.145
EOF
  touch "$TMP/firewall/direct-tcp-allow.txt"
  : > "$TMP/firewall/default-mode"

  # Inline mirror of migrate_legacy_firewall() in install.sh.
  local DEST="$TMP"
  local legacy_mode="$DEST/.configured-firewall-mode"
  local mode_file="$DEST/firewall/default-mode"
  local env_file="$DEST/.env"
  local tcp_file="$DEST/firewall/direct-tcp-allow.txt"
  [ -f "$legacy_mode" ] && [ ! -s "$mode_file" ] && cp "$legacy_mode" "$mode_file"
  if grep -q '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$env_file"; then
    local value
    value=$(grep '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$env_file" | head -1 | cut -d= -f2- | tr -d '"' | tr -d "'")
    if [ -n "$value" ]; then
      { echo ""; echo "# Migrated"; echo "$value" | tr ',' '\n' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'; } >> "$tcp_file"
    fi
    local tmp; tmp=$(mktemp); grep -v '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$env_file" > "$tmp" || true; mv "$tmp" "$env_file"
  fi

  assert_eq_file_content "$TMP/firewall/default-mode" "basic" "migration: mode -> basic"
  assert_contains "$TMP/firewall/direct-tcp-allow.txt" "claude-bridge:9223" "migration: tcp entry 1"
  assert_contains "$TMP/firewall/direct-tcp-allow.txt" "host:11434"          "migration: tcp entry 2"
  assert_false grep -q '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$TMP/.env" -- "migration: .env stripped"
}

run_tests
