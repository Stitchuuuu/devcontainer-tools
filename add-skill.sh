#!/usr/bin/env bash
# add-skill.sh — add ONE skill template to an already-bootstrapped project.
#
# Companion to install.sh (full bootstrap) and update.sh (full re-sync).
# Use this when the target project already has .devcontainer/ from devcontainer-
# tools and you want to add a single skill without re-running update.sh full-
# overwrite.
#
# Usage:
#   bash add-skill.sh                              # interactive: list available skills, prompt
#   bash add-skill.sh master-review                # add master-review to cwd
#   bash add-skill.sh master-review /path/to/proj  # add to non-cwd target
#   bash add-skill.sh master-review --force        # overwrite without prompt

set -euo pipefail

# -----------------------------------------------
# Colors & helpers (mirrored from install.sh)
# -----------------------------------------------
if [ -t 1 ]; then
	BOLD='\033[1m' DIM='\033[2m' GREEN='\033[0;32m'
	YELLOW='\033[0;33m' RED='\033[0;31m' CYAN='\033[0;36m' RESET='\033[0m'
else
	BOLD='' DIM='' GREEN='' YELLOW='' RED='' CYAN='' RESET=''
fi

info()    { echo -e "${CYAN}ℹ${RESET}  $1"; }
success() { echo -e "${GREEN}✓${RESET}  $1"; }
warn()    { echo -e "${YELLOW}⚠${RESET}  $1"; }
error()   { echo -e "${RED}✗${RESET}  $1" >&2; }
header()  { echo -e "\n${BOLD}=== $1 ===${RESET}\n"; }

# -----------------------------------------------
# Argument parsing
# -----------------------------------------------
SKILL_NAME=""
TARGET_DIR=""
FORCE=false
for arg in "$@"; do
	case "$arg" in
		--force) FORCE=true ;;
		--*)     error "unknown flag: $arg"; exit 2 ;;
		*)
			if [ -z "$SKILL_NAME" ]; then
				SKILL_NAME="$arg"
			elif [ -z "$TARGET_DIR" ]; then
				TARGET_DIR="$arg"
			else
				error "too many positional args"; exit 2
			fi
			;;
	esac
done

# -----------------------------------------------
# Locate template directory
# -----------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE_DIR="$SCRIPT_DIR/templates/v2"
SKILLS_TEMPLATE_DIR="$TEMPLATE_DIR/skills"

if [ ! -d "$SKILLS_TEMPLATE_DIR" ]; then
	error "Skills template directory not found: $SKILLS_TEMPLATE_DIR"
	exit 1
fi

# -----------------------------------------------
# Resolve target dir
# -----------------------------------------------
TARGET_DIR="${TARGET_DIR:-$(pwd)}"
TARGET_DIR="$(cd "$TARGET_DIR" 2>/dev/null && pwd || echo "$TARGET_DIR")"
DEST="$TARGET_DIR/.devcontainer"

if [ ! -d "$DEST" ]; then
	error "No .devcontainer/ found in $TARGET_DIR"
	error "Run install.sh first to bootstrap the project."
	exit 1
fi

# -----------------------------------------------
# List or validate skill name
# -----------------------------------------------
list_available() {
	# Filter out non-skill entries (e.g. sync-skills.sh)
	find "$SKILLS_TEMPLATE_DIR" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort
}

if [ -z "$SKILL_NAME" ]; then
	header "Available skills"
	available=$(list_available)
	if [ -z "$available" ]; then
		error "No skills found under $SKILLS_TEMPLATE_DIR"
		exit 1
	fi
	echo "$available" | nl -ba
	echo ""
	read -p "  Skill name (or number): " choice
	if [ -z "$choice" ]; then
		info "Aborted."
		exit 0
	fi
	# Numeric → resolve to name
	if [[ "$choice" =~ ^[0-9]+$ ]]; then
		SKILL_NAME=$(echo "$available" | sed -n "${choice}p")
	else
		SKILL_NAME="$choice"
	fi
fi

if [ ! -d "$SKILLS_TEMPLATE_DIR/$SKILL_NAME" ]; then
	error "No template for skill '$SKILL_NAME' under $SKILLS_TEMPLATE_DIR"
	echo ""
	info "Available:"
	list_available | sed 's/^/  - /'
	exit 1
fi

# -----------------------------------------------
# Conflict check
# -----------------------------------------------
SKILL_DEST="$DEST/skills/$SKILL_NAME"

if [ -d "$SKILL_DEST" ] && [ "$FORCE" = false ]; then
	warn "Destination already exists: $SKILL_DEST"
	read -p "  Overwrite (re-copy template files; user-customized files preserved if listed in copy_if_missing)? [y/N]: " _confirm
	case "$(echo "$_confirm" | tr '[:upper:]' '[:lower:]')" in
		y|yes) ;;
		*) info "Aborted."; exit 0 ;;
	esac
fi

# -----------------------------------------------
# Copy template files
# -----------------------------------------------
header "Installing skill: $SKILL_NAME"

mkdir -p "$SKILL_DEST"

# rsync would be nicer for "preserve some, overwrite others" but it's not
# always installed. We use a per-skill rule: any review-config.md (without
# .example) is preserved if already present (matches install.sh's
# copy_if_missing). Everything else is verbatim copied.
while IFS= read -r src; do
	rel="${src#$SKILLS_TEMPLATE_DIR/$SKILL_NAME/}"
	dst="$SKILL_DEST/$rel"
	mkdir -p "$(dirname "$dst")"

	# Per-file preservation rule
	case "$rel" in
		review-config.md)
			if [ -f "$dst" ]; then
				info "$rel (kept existing)"
				continue
			fi
			;;
	esac

	cp -p "$src" "$dst"
	success "$rel"
done < <(find "$SKILLS_TEMPLATE_DIR/$SKILL_NAME" -type f)

# Make .sh files executable
find "$SKILL_DEST" -type f -name '*.sh' -exec chmod +x {} +

# -----------------------------------------------
# Apply hint
# -----------------------------------------------
echo ""
success "Skill '$SKILL_NAME' installed to $SKILL_DEST"
echo ""
info "Apply now: ${BOLD}bash $DEST/skills/sync-skills.sh${RESET}"
info "Or: restart your devcontainer (post-start runs sync-skills.sh automatically)."

# Inside-devcontainer auto-apply prompt
if { [ -n "${REMOTE_CONTAINERS:-}" ] || [ -f /.dockerenv ]; } && [ "$TARGET_DIR" = "$(pwd)" ] && [ -x "$DEST/skills/sync-skills.sh" ]; then
	echo ""
	read -p "  Run sync-skills.sh now? [y/N]: " _apply
	if [ "$(echo "$_apply" | tr '[:upper:]' '[:lower:]')" = "y" ]; then
		bash "$DEST/skills/sync-skills.sh"
	fi
fi
