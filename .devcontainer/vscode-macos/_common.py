"""Shared utilities for vscode-macos/*.py patch scripts.

Runs on HOST (mac), not inside the devcontainer. Patches the installed
VS Code app bundle at /Applications/Visual Studio Code.app (default) or
a user-supplied path.

Surface
-------
- ANSI color constants + banner()
- resolve_app_bundle(argv): argv[1] or autodiscover VS Code app bundles
- resolve_target_file(app_root, needle): grep-based file lookup
- codesign_bundle(app_root): re-adhoc-sign after mutation, mandatory on
  hardened macOS to keep the launcher accepting the modified bundle.
- ensure_writable(target): chmod +w if needed
- snapshot_bundle_if_env(app_root): full bundle copy under
  ~/.devcontainer-focus-backups/ when $VSCODE_MACOS_SNAPSHOT is set.
"""

import json
import os
import sys
import shutil
import subprocess
from datetime import datetime
from pathlib import Path

RED, YELLOW, GREEN, BOLD, RESET = (
    "\033[31m", "\033[33m", "\033[32m", "\033[1m", "\033[0m"
)


def banner(title, reason, impact_lines):
    bar = "═" * 78
    out = [f"{RED}{BOLD}{bar}", f"  ⚠  {title}", f"  Reason: {reason}"]
    for line in impact_lines:
        out.append(f"  {line}")
    out.append(f"{bar}{RESET}")
    print("\n" + "\n".join(out) + "\n", file=sys.stderr)


DEFAULT_CANDIDATES = [
    "/Applications/Visual Studio Code.app",
    "/Applications/Visual Studio Code - Insiders.app",
    "/Applications/Cursor.app",
    "/Applications/Windsurf.app",
]


def resolve_app_bundle(argv):
    """Return Path to a VS Code (or fork) app bundle."""
    if len(argv) >= 2:
        p = Path(argv[1])
        if not p.exists():
            banner(
                "APP BUNDLE NOT FOUND",
                f"{p} does not exist",
                ["Provide a valid .app path"],
            )
            sys.exit(1)
        return p
    for c in DEFAULT_CANDIDATES:
        if Path(c).exists():
            print(f"→ Auto-discovered app bundle: {c}")
            return Path(c)
    banner(
        "NO VS CODE-LIKE APP FOUND",
        f"Checked: {', '.join(DEFAULT_CANDIDATES)}",
        ["Pass an explicit path as argv[1]"],
    )
    sys.exit(1)


def app_resources(app_root):
    """Return the app's Contents/Resources/app root — where the JS lives."""
    r = app_root / "Contents" / "Resources" / "app"
    if not r.is_dir():
        banner(
            "MALFORMED APP BUNDLE",
            f"{r} missing",
            ["Not a VS Code-style Electron bundle"],
        )
        sys.exit(1)
    return r


def resolve_target_file(app_resources_root, filename_hint, content_needle):
    """
    Locate a specific JS file inside the app.
    - filename_hint : basename to search (e.g. `windowImpl.js`).
    - content_needle : string that must appear in the file (e.g. `FocusMode`).
                       Guards against picking wrong compiled artifact.
    """
    candidates = list(app_resources_root.rglob(filename_hint))
    for c in candidates:
        try:
            if content_needle in c.read_text(errors="ignore"):
                return c
        except Exception:
            continue
    # Fallback : scan all JS for the needle
    for c in app_resources_root.rglob("*.js"):
        try:
            if content_needle in c.read_text(errors="ignore"):
                # Only accept if filename hint appears in path
                if filename_hint in str(c):
                    return c
        except Exception:
            continue
    return None


def ensure_writable(path):
    if not os.access(path, os.W_OK):
        try:
            path.chmod(path.stat().st_mode | 0o200)
        except PermissionError:
            banner(
                "PERMISSION DENIED",
                f"Cannot chmod +w {path}",
                ["Run with sudo, or chown the app bundle first"],
            )
            sys.exit(1)


SNAPSHOT_ENV = "VSCODE_MACOS_SNAPSHOT"
SNAPSHOT_DIR = Path.home() / ".devcontainer-focus-backups"


def _bundle_version(app_root):
    """Best-effort read of the bundle's product version from package.json."""
    pkg = app_root / "Contents" / "Resources" / "app" / "package.json"
    try:
        return json.loads(pkg.read_text()).get("version", "unknown")
    except Exception:
        return "unknown"


def snapshot_bundle_if_env(app_root):
    """
    If $VSCODE_MACOS_SNAPSHOT is truthy, copy the whole app bundle to
    ~/.devcontainer-focus-backups/<name>.<version>-<timestamp>/ using
    `ditto` (macOS-native, preserves xattrs, symlinks, resource forks,
    codesign metadata).

    Skipped silently when the env var is unset. Skipped with a note
    when a snapshot for the same (bundle-name, version) already exists
    — one-shot per install version, safe to re-invoke.

    Rationale : file-level `.bak` covers the mutated JS, but a botched
    codesign or an aborted test iteration can leave the whole bundle
    unlaunchable. A full snapshot means rollback = one `ditto` back,
    no App Store / installer round-trip.
    """
    val = os.environ.get(SNAPSHOT_ENV, "").strip().lower()
    if val in ("", "0", "false", "no", "off"):
        return None

    version = _bundle_version(app_root)
    prefix = f"{app_root.name}.{version}-"
    SNAPSHOT_DIR.mkdir(parents=True, exist_ok=True)

    existing = sorted(SNAPSHOT_DIR.glob(f"{prefix}*"))
    if existing:
        print(
            f"→ Bundle snapshot already present for {app_root.name} "
            f"v{version} : {existing[-1].name} (skip)"
        )
        return existing[-1]

    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    dest = SNAPSHOT_DIR / f"{prefix}{ts}"
    print(f"→ Snapshotting bundle to {dest} (via ditto)…")
    try:
        result = subprocess.run(
            ["ditto", str(app_root), str(dest)],
            capture_output=True, text=True,
        )
    except FileNotFoundError:
        banner(
            "SNAPSHOT UNAVAILABLE",
            "`ditto` not found on PATH — this script must run on macOS host.",
            [
                "Continuing without full snapshot.",
                "Per-file .bak remains available for rollback.",
            ],
        )
        return None
    if result.returncode != 0:
        banner(
            "SNAPSHOT FAILED",
            result.stderr.strip() or "unknown error",
            [
                "Continuing without full snapshot.",
                "Per-file .bak remains available for rollback.",
            ],
        )
        return None
    print(f"{GREEN}  ✓ snapshot ready{RESET}")
    return dest


LSREGISTER = (
    "/System/Library/Frameworks/CoreServices.framework/Versions/A"
    "/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister"
)


def _lsregister_refresh(app_root):
    """
    Force LaunchServices to re-scan Info.plist so the bundle's URL scheme
    handlers (`vscode-remote://`, `vscode://`, …) and document type
    associations stay registered after the ad-hoc re-sign.

    Symptom this prevents : `open "vscode-remote://…"` fails with
    `kLSApplicationNotFoundErr` because ad-hoc signing changes the code
    directory hash, and LaunchServices drops the previous registration
    that was keyed on the Microsoft Developer ID signature.
    """
    print(f"→ Re-registering with LaunchServices…")
    try:
        result = subprocess.run(
            [LSREGISTER, "-f", str(app_root)],
            capture_output=True, text=True,
        )
    except FileNotFoundError:
        banner(
            "LSREGISTER MISSING",
            f"{LSREGISTER} not on disk — not a macOS host ?",
            [
                "URL scheme handlers may need manual re-registration.",
            ],
        )
        return False
    if result.returncode != 0:
        banner(
            "LSREGISTER FAILED",
            result.stderr.strip() or f"exit {result.returncode}",
            [
                "`open vscode-remote://…` may fail with kLSApplicationNotFoundErr.",
                "Manual fix : run the lsregister -f command listed in README.",
            ],
        )
        return False
    print(f"{GREEN}  ✓ URL schemes re-registered{RESET}")
    return True


def codesign_bundle(app_root):
    """
    Re-adhoc-sign the whole bundle then refresh LaunchServices.
    Required after mutating any file inside Contents/ — macOS rejects
    launch with `code signature invalid` if the Info.plist hash
    disagrees with the on-disk state. The lsregister step keeps URL
    scheme handlers alive across the signature change.
    """
    print(f"\n→ Re-signing {app_root} (ad-hoc)…")
    result = subprocess.run(
        ["codesign", "--sign", "-", "--deep", "--force",
         "--preserve-metadata=entitlements,requirements", str(app_root)],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        banner(
            "CODESIGN FAILED",
            result.stderr.strip() or "unknown error",
            [
                "Bundle may fail to launch (\"code signature invalid\").",
                "Try : xattr -cr <bundle>  then re-run this script.",
            ],
        )
        return False
    print(f"{GREEN}  ✓ signed{RESET}")
    _lsregister_refresh(app_root)
    return True
