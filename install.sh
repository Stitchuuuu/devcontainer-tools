#!/usr/bin/env bash
# DevContainer Template Installer — v2
# Drops the v2 baseline (~95 files) into a project's .devcontainer/.
set -euo pipefail

TEMPLATE_VERSION="2.1.0"

# -----------------------------------------------
# Colors & messaging
# -----------------------------------------------
if [ -t 1 ]; then
    BOLD=$'\033[1m'; DIM=$'\033[2m'
    GREEN=$'\033[0;32m'; YELLOW=$'\033[0;33m'; RED=$'\033[0;31m'; CYAN=$'\033[0;36m'
    RESET=$'\033[0m'
else
    BOLD='' DIM='' GREEN='' YELLOW='' RED='' CYAN='' RESET=''
fi

info()    { echo -e "${CYAN}ℹ${RESET}  $1"; }
success() { echo -e "${GREEN}✓${RESET}  $1"; }
warn()    { echo -e "${YELLOW}⚠${RESET}  $1"; }
error()   { echo -e "${RED}✗${RESET}  $1" >&2; }
header()  { echo -e "\n${BOLD}=== $1 ===${RESET}\n"; }

# -----------------------------------------------
# Generic bash helpers
# -----------------------------------------------
ask() {
    local prompt="$1" default="$2" var="$3"
    if [ -n "$default" ]; then
        read -r -p "  $prompt [$default]: " _val
        eval "$var=\"\${_val:-$default}\""
    else
        read -r -p "  $prompt: " _val
        eval "$var=\"\$_val\""
    fi
}

slugify() {
    echo "$1" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-//; s/-$//'
}

titlecase() {
    echo "$1" | tr '-' ' ' | awk '{for(i=1;i<=NF;i++) $i=toupper(substr($i,1,1)) substr($i,2)}1'
}

# Escape sed replacement chars (|, &, \)
sed_escape() {
    printf '%s' "$1" | sed -e 's/[\/&|\\]/\\&/g'
}

# Portable in-place sed (BSD vs GNU)
if sed --version >/dev/null 2>&1; then
    sedi() { sed -i "$@"; }
else
    sedi() { sed -i '' "$@"; }
fi

# -----------------------------------------------
# Copy helpers (operate relative to TEMPLATE_DIR and DEST)
# -----------------------------------------------
copy_verbatim() {
    # $1 = relative path under templates/ (and under DEST)
    local src="$TEMPLATE_DIR/$1" dst="$DEST/$1"
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
}

copy_templated() {
    # $1 = relative path ; runs sed for the 2 placeholders
    local src="$TEMPLATE_DIR/$1" dst="$DEST/$1"
    mkdir -p "$(dirname "$dst")"
    sed -e "s|{{PROJECT_ID}}|${PROJECT_ID_ESC}|g" \
        -e "s|{{PROJECT_DISPLAY_NAME}}|${DISPLAY_NAME_ESC}|g" \
        "$src" > "$dst"
}

copy_templated_as() {
    # $1 = relative src ; $2 = relative dst (when src and dst names differ —
    # e.g. Dockerfile.php → Dockerfile for PHP installs). Same sed pipeline
    # as copy_templated().
    local src="$TEMPLATE_DIR/$1" dst="$DEST/$2"
    mkdir -p "$(dirname "$dst")"
    sed -e "s|{{PROJECT_ID}}|${PROJECT_ID_ESC}|g" \
        -e "s|{{PROJECT_DISPLAY_NAME}}|${DISPLAY_NAME_ESC}|g" \
        "$src" > "$dst"
}

copy_dir() {
    # $1 = relative dir ; recursive copy preserving structure
    local src="$TEMPLATE_DIR/$1" dst="$DEST/$1"
    mkdir -p "$dst"
    cp -r "$src/." "$dst/"
}

chmod_exec() {
    chmod +x "$@" 2>/dev/null || true
}

# -----------------------------------------------
# Locate template directory
# -----------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# templates/ is plural to allow future flavours — currently only v2 ships.
# Override with TEMPLATE_VARIANT=<name> to switch (when alternatives exist).
TEMPLATE_VARIANT="${TEMPLATE_VARIANT:-v2}"
TEMPLATE_DIR="$SCRIPT_DIR/templates/$TEMPLATE_VARIANT"
if [ ! -d "$TEMPLATE_DIR" ]; then
    error "Template directory not found: $TEMPLATE_DIR"
    error "  (TEMPLATE_VARIANT=$TEMPLATE_VARIANT — set the env var to pick a different one)"
    exit 1
fi

# -----------------------------------------------
# Wizard phases
# -----------------------------------------------
banner() {
    cat <<BANNER
${BOLD}DevContainer Template Installer v${TEMPLATE_VERSION}${RESET}
${DIM}Drops the v2 baseline (Claude Code + firewall + skills + sidecar)
into a target project's .devcontainer/.${RESET}

BANNER
}

resolve_target_dir() {
    # CLI arg overrides ; falls back to current dir.
    TARGET_DIR="${1:-$(pwd)}"
    if [ ! -d "$TARGET_DIR" ]; then
        error "Target directory does not exist: $TARGET_DIR"
        exit 1
    fi
    TARGET_DIR="$(cd "$TARGET_DIR" && pwd)"
    DEST="$TARGET_DIR/.devcontainer"
}

_offer_reinstall_or_abort() {
    warn ".devcontainer/ already exists at $TARGET_DIR"
    echo "  [1] Reinstall (overwrite files ; preserve .env if present)"
    echo "  [2] Abort"
    read -r -p "  Choose [1/2] (default: 2): " _c
    case "${_c:-2}" in
        1) info "Proceeding with reinstall — .env will be preserved if present." ;;
        *) info "Aborted."; exit 0 ;;
    esac
}

detect_existing_devcontainer() {
    [ -d "$DEST" ] || return 0

    if [ -f "$DEST/.configured-setup" ]; then
        local _v
        _v="$(grep -E '^VERSION=' "$DEST/.configured-setup" 2>/dev/null \
              | head -1 | sed -E 's/^VERSION=//; s/"//g' || echo unknown)"
        case "$_v" in
            2.*) _offer_reinstall_or_abort ;;
            1.*|unknown)
                error "Detected legacy v1 devcontainer (marker VERSION=$_v)"
                echo
                echo "  install.sh v2 does NOT auto-migrate from v1.3."
                echo "  The Part 2 migration prompt is not yet specced — see"
                echo "    /workspace/plans/devcontainer-tools-v2-migration/ROLLOUT.md"
                echo "  Workarounds : back up .env, rm -rf .devcontainer, re-run install."
                exit 1
                ;;
        esac
    else
        # .devcontainer/ exists but no marker — treat as partial v2
        _offer_reinstall_or_abort
    fi
}

wizard_project_id() {
    header "Project identifier"
    local _default _slug_re
    _default="$(slugify "$(basename "$TARGET_DIR")")"
    _slug_re='^[a-z0-9]([a-z0-9-]*[a-z0-9])?$'
    ask "Project slug (used as DC_PROJECT in .env)" "$_default" PROJECT_ID
    if ! echo "$PROJECT_ID" | grep -qE "$_slug_re"; then
        error "Invalid slug — lowercase letters, digits, hyphens ; cannot start/end with '-'"
        exit 1
    fi
}

wizard_display_name() {
    header "Display name"
    local _default
    _default="$(titlecase "$PROJECT_ID")"
    ask "Display name (for VS Code title)" "$_default" PROJECT_DISPLAY_NAME
}

wizard_project_type() {
    header "Project type"
    echo "  [1] Node.js   (default — generic project layer, FROM base)"
    echo "  [2] PHP       (PHP 8.2 + Composer, FROM base)"
    echo "  [3] Custom    (base only, edit Dockerfile post-install)"
    read -r -p "  Choose [1/2/3] (default: 1): " _t
    case "${_t:-1}" in
        1) PROJECT_TYPE="node" ;;
        2) PROJECT_TYPE="php"  ;;
        3) PROJECT_TYPE="custom" ;;
        *) error "Invalid choice"; exit 1 ;;
    esac
}

wizard_creds_volume() {
    header "Shared Claude credentials volume"
    echo "  Sharing the OAuth volume across devcontainers means one login"
    echo "  per machine instead of per project."
    echo ""
    local _found _default _v
    _found="$(docker volume ls --format '{{.Name}}' 2>/dev/null \
              | grep -E '^claude-creds-' | sort -u || true)"
    if [ -n "$_found" ]; then
        echo "  Existing claude-creds-* volumes on this host :"
        echo "$_found" | sed 's/^/    /'
        echo ""
        _default="$(echo "$_found" | head -1)"
    else
        info "No existing claude-creds-* volume found."
        _default="claude-credentials-shared"
    fi
    echo "  Enter a volume name (created if absent), or 'n' for per-project isolation."
    read -r -p "  Volume [$_default]: " _v
    _v="${_v:-$_default}"
    case "$(echo "$_v" | tr 'A-Z' 'a-z')" in
        n|no|none) CLAUDE_CREDS_VOLUME="" ;;
        *)         CLAUDE_CREDS_VOLUME="$_v" ;;
    esac
}

summary_and_confirm() {
    header "Summary"
    echo "  Target          : $TARGET_DIR"
    echo "  Project ID      : $PROJECT_ID"
    echo "  Display name    : $PROJECT_DISPLAY_NAME"
    echo "  Project type    : $PROJECT_TYPE"
    if [ -n "$CLAUDE_CREDS_VOLUME" ]; then
        echo "  Creds volume    : $CLAUDE_CREDS_VOLUME (shared)"
    else
        echo "  Creds volume    : (per-project — DC_PROJECT-derived default)"
    fi
    echo ""
    read -r -p "  Proceed ? [Y/n]: " _ok
    case "${_ok:-y}" in
        n|N|no|No) info "Aborted."; exit 0 ;;
    esac
}

# -----------------------------------------------
# Install phase
# -----------------------------------------------
install_files() {
    header "Installing v$TEMPLATE_VERSION baseline → $DEST"
    mkdir -p "$DEST"

    # ── Templated (2 files, 2 placeholders) ────────────────────
    copy_templated devcontainer.json
    copy_templated .env.example

    # ── Build ──────────────────────────────────────────────────
    copy_verbatim Dockerfile.base
    case "$PROJECT_TYPE" in
        node|custom) copy_templated_as Dockerfile     Dockerfile ;;
        php)         copy_templated_as Dockerfile.php Dockerfile ;;
    esac
    copy_templated docker-compose.yml
    copy_verbatim vscode-settings.jsonc

    # ── Lifecycle (6) ──────────────────────────────────────────
    copy_templated initialize.sh
    for f in on-create post-create post-start shell-init install-extensions; do
        copy_verbatim "${f}.sh"
    done

    # ── Zsh base + per-dev override example (sourced by shell-init.sh) ─
    copy_verbatim zshrc-base
    copy_verbatim zshrc.local.example

    # ── Firewall core (9) ──────────────────────────────────────
    for f in init-firewall.sh firewall-mode.sh test-firewall.sh; do
        copy_verbatim "$f"
    done
    for f in dnsmasq.conf compile-policy.py mitm-init.sh domains.txt \
             domains.local.txt.example firewall-blocks \
             default-mode direct-tcp-allow.txt \
             firewall-docker-setup.sh; do
        copy_verbatim "firewall/$f"
    done
    # ── Firewall trees (5 dirs) ────────────────────────────────
    copy_dir firewall/addons
    copy_dir firewall/policy.d
    copy_dir firewall/policy.local.d.example
    copy_dir firewall/policy.local.d
    copy_dir firewall/tests

    # ── Notify daemon (host-side, runs while container is up) ──
    # vscode-ext-patchs/ rides on copy_dir claude (recursive) below.
    copy_dir notify
    copy_templated notify/tests/PROMPTS.md
    copy_templated notify/tests/winrt-standalone.js

    # ── Claude (5 files + vscode-ext-patchs/ via recursive copy_dir) ──
    copy_dir claude
    copy_templated claude/CLAUDE-project.md

    # ── Knowledge (full dir) ───────────────────────────────────
    copy_dir knowledge

    # ── Docs (4 files) ─────────────────────────────────────────
    copy_templated RESEARCH.md
    for f in README RUNBOOK SECURITY; do
        copy_verbatim "${f}.md"
    done

    # ── Local-backend sidecar ──────────────────────────────────
    copy_dir claude-bridge
    copy_dir host-helpers
    copy_templated host-helpers/research-cleanup
    copy_templated host-helpers/docker-audit.sh
    copy_verbatim diag-ollama-local.sh

    # ── Skills (sync + 5 generic) ──────────────────────────────
    copy_verbatim skills/sync-skills.sh
    for s in prepare-pr watch-log prepare-research scan-deps prepare-plan notify-queue diagram; do
        copy_dir "skills/$s"
    done
    copy_templated skills/prepare-research/prepare-research.skill.md

    # ── Tests (unit + integration suites) ──────────────────────
    copy_dir tests

    # ── Gitignore (.devcontainer/-scope rules) ─────────────────
    copy_verbatim .gitignore

    # ── Dockerignore (build-context exclusions) ────────────────
    copy_verbatim .dockerignore

    # ── Gitignore-root (root-scope fragment ; appended to
    #    <target>/.gitignore by update_gitignore) ───────────────
    copy_verbatim .gitignore-root

    # ── LESSONS baseline (preserve user content on re-install) ─
    [ -f "$DEST/LESSONS.md" ] || cp "$TEMPLATE_DIR/LESSONS.md" "$DEST/LESSONS.md"

    success "Baseline installed"
}

# -----------------------------------------------
# Post-install : .env, .gitignore, exec perms, marker
# -----------------------------------------------
migrate_legacy_firewall() {
    # One-shot migration of legacy firewall config to baked files.
    # Pre-bake (before session 1) the firewall mode lived at
    # <project>/.devcontainer/.configured-firewall-mode and the direct-TCP
    # allowlist lived as CLAUDE_CODE_FIREWALL_ALLOWED=host:port,host:port,…
    # in .env. Both were workspace-mutable at runtime → bypass surface.
    # Now baked into firewall/default-mode + firewall/direct-tcp-allow.txt.
    # Idempotent : re-running does nothing once migrated.
    local legacy_mode="$DEST/.configured-firewall-mode"
    local mode_file="$DEST/firewall/default-mode"
    local env_file="$DEST/.env"
    local tcp_file="$DEST/firewall/direct-tcp-allow.txt"
    local moved=0

    if [ -f "$legacy_mode" ] && [ ! -s "$mode_file" ]; then
        cp "$legacy_mode" "$mode_file"
        info "Migrated firewall mode : .configured-firewall-mode → firewall/default-mode"
        moved=1
    fi

    if [ -f "$env_file" ] && grep -q '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$env_file"; then
        local value
        value=$(grep '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$env_file" | head -1 \
                  | cut -d= -f2- | tr -d '"' | tr -d "'")
        if [ -n "$value" ]; then
            {
                echo ""
                echo "# Migrated from .env CLAUDE_CODE_FIREWALL_ALLOWED on $(date +%Y-%m-%d)"
                echo "$value" | tr ',' '\n' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
            } >> "$tcp_file"
            info "Migrated CLAUDE_CODE_FIREWALL_ALLOWED='$value' → firewall/direct-tcp-allow.txt"
            moved=1
        fi
        # Strip the line from .env regardless (idempotent, BSD/GNU compat).
        local tmp; tmp=$(mktemp)
        grep -v '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$env_file" > "$tmp" || true
        mv "$tmp" "$env_file"
    fi

    if [ "$moved" -eq 1 ]; then
        success "Legacy firewall config migrated to baked files"
    fi
}

generate_env() {
    local env="$DEST/.env"
    if [ -f "$env" ]; then
        success ".env preserved from previous install"
        return 0
    fi
    cp "$DEST/.env.example" "$env"
    # Uncomment the templated DC_PROJECT line (becomes `#DC_PROJECT=<PROJECT_ID>`
    # after `copy_templated`)
    sedi -E "s|^#DC_PROJECT=${PROJECT_ID_ESC}\$|DC_PROJECT=${PROJECT_ID_ESC}|" "$env"
    # CLAUDE_CREDS_VOLUME : set if shared, leave commented otherwise
    if [ -n "$CLAUDE_CREDS_VOLUME" ]; then
        local _esc; _esc="$(sed_escape "$CLAUDE_CREDS_VOLUME")"
        sedi -E "s|^#?CLAUDE_CREDS_VOLUME=.*|CLAUDE_CREDS_VOLUME=${_esc}|" "$env"
    fi
    success ".env created from .env.example"
}

update_gitignore() {
    # Source : <DEST>/.gitignore-root (shipped via install_files from
    # templates/<variant>/.gitignore-root, root-scope fragment). Appended
    # verbatim ; first line acts as a sentinel for idempotent re-runs.
    # NB : .devcontainer/-scope rules ship as .devcontainer/.gitignore
    # directly (copy_verbatim), not appended to root.
    local gi="$TARGET_DIR/.gitignore"
    local src="$DEST/.gitignore-root"
    touch "$gi"

    if [ ! -f "$src" ]; then
        warn ".gitignore template not found at $src — skipping"
        return 0
    fi

    local sentinel
    sentinel="$(head -n1 "$src")"
    if grep -qxF -- "$sentinel" "$gi" 2>/dev/null; then
        success ".gitignore already up to date"
        return 0
    fi

    # Separate from any pre-existing project content with a blank line.
    [ -s "$gi" ] && echo "" >> "$gi"
    cat "$src" >> "$gi"
    success ".gitignore updated (template baseline appended)"
}

set_exec_perms() {
    # Root .sh
    chmod_exec "$DEST"/initialize.sh "$DEST"/on-create.sh "$DEST"/post-create.sh \
               "$DEST"/post-start.sh "$DEST"/shell-init.sh "$DEST"/install-extensions.sh
    chmod_exec "$DEST"/init-firewall.sh "$DEST"/firewall-mode.sh "$DEST"/test-firewall.sh
    chmod_exec "$DEST"/diag-ollama-local.sh

    # Firewall internals
    chmod_exec "$DEST"/firewall/mitm-init.sh
    chmod_exec "$DEST"/firewall/firewall-blocks
    chmod_exec "$DEST"/firewall/compile-policy.py
    chmod_exec "$DEST"/firewall/firewall-docker-setup.sh

    # Claude
    chmod_exec "$DEST"/claude/sync-creds.sh

    # claude-bridge
    chmod_exec "$DEST"/claude-bridge/healthcheck.sh

    # host-helpers (all 12 are extensionless executables)
    chmod_exec "$DEST"/host-helpers/*

    # Skills
    chmod_exec "$DEST"/skills/sync-skills.sh
    if [ -d "$DEST/skills/scan-deps" ]; then
        [ -f "$DEST/skills/scan-deps/extract-auto-dependencies" ] && \
            chmod_exec "$DEST"/skills/scan-deps/extract-auto-dependencies
        if [ -d "$DEST/skills/scan-deps/extractors" ]; then
            chmod_exec "$DEST"/skills/scan-deps/extractors/*
        fi
    fi

    success "Executable permissions set"
}

write_v2_marker() {
    cat > "$DEST/.configured-setup" <<EOF
# Auto-generated by install.sh v$TEMPLATE_VERSION — do not edit.
VERSION="$TEMPLATE_VERSION"
PROJECT_ID="$PROJECT_ID"
PROJECT_DISPLAY_NAME="$PROJECT_DISPLAY_NAME"
PROJECT_TYPE="$PROJECT_TYPE"
CLAUDE_CREDS_VOLUME="$CLAUDE_CREDS_VOLUME"
INSTALLED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
EOF
    success ".configured-setup written (v$TEMPLATE_VERSION)"
}

link_lessons_root() {
    # Symlink at the project root for visibility — same pattern as CLAUDE.md
    # (cf. post-create.sh). Commits as git mode 120000.
    ln -sf ".devcontainer/LESSONS.md" "$TARGET_DIR/LESSONS.md"
    success "LESSONS.md → .devcontainer/LESSONS.md (symlink)"
}

install_claude_settings() {
    # Pre-approved read-only permissions baseline at project root.
    # .example is always refreshed (canonical reference, tracked in git
    # via counter-exclusion in .gitignore-root). The live file is only
    # written when missing, preserving any user customisations on
    # re-install — same idempotency pattern as LESSONS.md above.
    local src="$TEMPLATE_DIR/.claude/settings.local.json.example"
    mkdir -p "$TARGET_DIR/.claude"
    cp "$src" "$TARGET_DIR/.claude/settings.local.json.example"
    [ -f "$TARGET_DIR/.claude/settings.local.json" ] \
        || cp "$src" "$TARGET_DIR/.claude/settings.local.json"
    success ".claude/settings.local.json{,.example} (read-only baseline)"
}

link_claude_rules_root() {
    # Expose .devcontainer/claude/CLAUDE-{dev,project}.md under
    # .claude/rules/{mandatory,project}.md so the claude CLI discovers
    # them even when launched outside the devcontainer. Counter-exclusion
    # in .gitignore-root keeps these two symlinks tracked while the rest
    # of .claude/ stays ignored. Commits as git mode 120000.
    mkdir -p "$TARGET_DIR/.claude/rules"
    ln -sf "../../.devcontainer/claude/CLAUDE-dev.md" \
           "$TARGET_DIR/.claude/rules/mandatory.md"
    ln -sf "../../.devcontainer/claude/CLAUDE-project.md" \
           "$TARGET_DIR/.claude/rules/project.md"
    success ".claude/rules/{mandatory,project}.md → .devcontainer/claude/CLAUDE-{dev,project}.md (symlinks)"
}

final_summary() {
    header "Done"
    cat <<SUMMARY
  ${GREEN}✓${RESET} .devcontainer/ installed at $DEST
  ${GREEN}✓${RESET} Project ID    : $PROJECT_ID
  ${GREEN}✓${RESET} Display name  : $PROJECT_DISPLAY_NAME
  ${GREEN}✓${RESET} Project type  : $PROJECT_TYPE
  ${GREEN}✓${RESET} Marker        : .devcontainer/.configured-setup (v$TEMPLATE_VERSION)

${BOLD}Next steps${RESET}

  1. Open the project in VS Code :
     ${CYAN}code "$TARGET_DIR"${RESET}

  2. Run "Reopen in Container" from the Command Palette
     (${DIM}Cmd/Ctrl+Shift+P → "Dev Containers: Reopen in Container"${RESET})

  3. First boot builds the base image (~5 min, one-off per
     CLAUDE_CODE_VERSION). Subsequent boots reuse the cached image.

  4. Optional ${CYAN}.env${RESET} tweaks (post-install, gitignored) :
     - ${CYAN}EXTRA_NETWORK${RESET}              — attach to an external docker network
     - ${CYAN}CLAUDE_CODE_FIREWALL_ALLOWED${RESET} — extend allowed host:port (firewall)
     - firewall mode : flip via ${CYAN}.devcontainer/firewall-mode.sh${RESET}

  5. Read ${CYAN}.devcontainer/README.md${RESET} for the full operational reference.

SUMMARY
}

# -----------------------------------------------
# Main
# -----------------------------------------------
main() {
    banner
    resolve_target_dir "$@"
    detect_existing_devcontainer

    wizard_project_id
    wizard_display_name
    wizard_project_type
    wizard_creds_volume
    summary_and_confirm

    # Pre-compute sed-safe escapes for the 2 placeholders
    PROJECT_ID_ESC="$(sed_escape "$PROJECT_ID")"
    DISPLAY_NAME_ESC="$(sed_escape "$PROJECT_DISPLAY_NAME")"

    install_files
    migrate_legacy_firewall
    generate_env
    update_gitignore
    set_exec_perms
    write_v2_marker
    link_lessons_root
    install_claude_settings
    link_claude_rules_root
    final_summary
}

main "$@"
