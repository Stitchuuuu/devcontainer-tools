# How to install : Capacitor Android plugin devcontainer

> Quick guide to bootstrap a Capacitor plugin project with this template's
> `CapacitorAndroid` variant. Assumes you've cloned the template repo
> alongside your target plugin project.

## Quick checklist (5 steps)

1. Scaffold your target plugin dir with `devc init` (the v3 CLI)
2. Copy `Dockerfile.CapacitorAndroidMin` (or `Dockerfile.CapacitorAndroidStd`) next to it and point the compose `build.dockerfile` at it
3. Paste sections from [`claude/CLAUDE-project-android-capacitor.md`](claude/CLAUDE-project-android-capacitor.md) into your project's `.devcontainer/claude/CLAUDE-project.md`
4. Paste sections from [`firewall/domains.capacitor-android.txt`](firewall/domains.capacitor-android.txt) into your project's `.devcontainer/firewall/domains.local.txt` (personal, gitignored) or `domains.txt` (team-committed)
5. Add `cache/*` to `.devcontainer/.gitignore` (if not already there)

Then `code your-project` — VSCode boots the devcontainer, Claude reads
`CLAUDE-project.md`, `kchk` / `kchk-matrix` / `fetch-android-lib` /
`capacitor-plugin-check.sh` are on PATH.

## Detailed steps

### 1. Scaffold and variant

In your plugin project :

    npx --yes @meitogi/devcontainer-cli devc init

The wizard asks for the project id (kebab-case, e.g. `my-plugin`), the display
name, the stack and the Claude credentials volume, and writes `.devcontainer/`.
Then copy the chosen `Dockerfile.CapacitorAndroidMin` or
`Dockerfile.CapacitorAndroidStd` from this repo's `.devcontainer/` into yours
and set `build.dockerfile` to it in `.devcontainer/docker-compose.yml`. The
image's own recipe for the stack is `stacks/android-capacitor.md` in the
`devcontainer-sandbox` repo.

### 2. Variant choice — Min vs Std

| | Min | Std |
|---|---|---|
| JDK | Debian OpenJDK 17 (apt) | Azul Zulu 17 (vendor apt) |
| Image size | ~1.81GB (base + Android tools) | ~1.82GB (base + Zulu + Android tools) |
| Best for | Fast startup, standard packaging | Vendor-canonical layout, easier stack traces |

Both share identical helpers, baked APIs (23/24/26/28/31/33/34/35),
Capacitor SDK + AndroidX + kotlinx deps.

### 3. Claude rules — copy sections

Open `claude/CLAUDE-project-android-capacitor.md`, copy the
sections you need into your project's `.devcontainer/claude/CLAUDE-project.md`.
Key sections :

- **Quick refs** — 6 command lines Claude uses (kchk / kchk-matrix / …)
- **Rules** — 4 rules incl. "every delivery runs the gate", multi-file
  compile, fetch missing libs
- **Details** — reference material Claude falls back to for edge cases

Paste as-is or adapt to your project's conventions.

### 4. Firewall allowlist — copy sections

Open `firewall/domains.capacitor-android.txt`. Two levels :

- **Level A** (docs) — reference lookup only (developer.android.com,
  kotlinlang.org, capacitorjs.com, …)
- **Level B** (Maven) — jar fetching (dl.google.com, repo.maven.apache.org, …)
- **Capacitor + npm** — capacitorjs.com, ionic.io, registry.npmjs.org

Paste desired blocks into :
- `.devcontainer/firewall/domains.local.txt` for **personal / temporary**
  overrides (gitignored, won't survive team share)
- `.devcontainer/firewall/domains.txt` for **team-committed** allowlist

After edit, **rebuild the devcontainer** — firewall daemon reloads
only at container boot (see `.devcontainer/firewall/CLAUDE.md`).

### 5. Gitignore cache dir

Add `cache/*` to `.devcontainer/.gitignore` so on-demand fetched jars
(retrofit, room, etc.) don't leak into commits :

    # .devcontainer/.gitignore
    pending/*
    cache/*

### 6. Ship the delivery gate script

`devc init` ships no `scripts/` ; copy the gate from this repo :

    cp scripts/capacitor-plugin-check.sh \
       your-project/.devcontainer/scripts/capacitor-plugin-check.sh
    chmod +x your-project/.devcontainer/scripts/capacitor-plugin-check.sh

## Daily workflow (once installed)

Inside the devcontainer (Claude's env) :

    # Compile-check a single file (API 35 default)
    kchk android/src/main/java/com/example/MyPlugin.kt

    # Matrix check across all baked APIs
    kchk-matrix android/src/main/java/com/example/MyPlugin.kt

    # Add a dep on-demand (persists to .devcontainer/cache/android-jars/maven/)
    fetch-android-lib com.squareup.retrofit2:retrofit:2.11.0

    # Before commit / PR — the delivery gate
    bash .devcontainer/scripts/capacitor-plugin-check.sh

## Extending to another API level

If your `minSdk` is API 21 (not in baked matrix) :

    fetch-android-api 21     # caches api-21.jar in .devcontainer/cache/android-jars/apis/
    kchk-api 21 MyPlugin.kt  # now available

Same pattern for any API level Google publishes.

## Related files (in this template)

- [`Dockerfile.CapacitorAndroidMin`](Dockerfile.CapacitorAndroidMin)
- [`Dockerfile.CapacitorAndroidStd`](Dockerfile.CapacitorAndroidStd)
- [`Dockerfile.AndroidMin`](Dockerfile.AndroidMin) / [`Dockerfile.AndroidStd`](Dockerfile.AndroidStd) — generic (non-Capacitor) Android
- [`claude/CLAUDE-project-android-capacitor.md`](claude/CLAUDE-project-android-capacitor.md)
- [`claude/CLAUDE-project-android.md`](claude/CLAUDE-project-android.md) — base Android rules (no Capacitor)
- [`firewall/domains.capacitor-android.txt`](firewall/domains.capacitor-android.txt)
- [`firewall/domains.android.txt`](firewall/domains.android.txt) — base Android allowlist
- [`scripts/capacitor-plugin-check.sh`](scripts/capacitor-plugin-check.sh)
