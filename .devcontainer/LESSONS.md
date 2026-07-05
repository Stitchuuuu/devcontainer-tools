# LESSONS — project-wide patterns (committed)

> Sister files : `LESSONS.local.md` (gitignored) for personal /
> not-yet-generalisable lessons. Cross-project preferences live in
> `~/.claude/memory/` (auto-memory).

<!-- Entries : one bullet per lesson. Rule first, then *Why* and
     *How to apply* on the same or following line. -->

- **Before editing any `.devcontainer/firewall/*` source file, read
  `.devcontainer/knowledge/firewall.md` end-to-end.** *Why* : the runtime
  config in `/var/run/devcontainer-firewall/` is root-owned and emitted only
  by `init-firewall.sh` at container boot — mid-session edits to
  `domains.txt` / `policy.d/*.yaml` don't refresh dnsmasq / ipset / mitmproxy.
  Running `python3 compile-policy.py` as `node` either fails on `/var/run/`
  writes or recompiles into a file no daemon re-reads, so verifying that way
  is a false signal. *How to apply* : edit the source → the only verification
  path is **rebuilding the devcontainer**. Never claim « tested by recompile »
  without rebuild. Knowledge at `knowledge/firewall.md` L138-188 documents
  the init flow and compile-policy modes.

- **Read `.devcontainer/firewall/policy.d/<host>.yaml` BEFORE any external
  network call (curl / gh / WebFetch / pip / npm) that targets a non-trivial
  host.** *Why* : this repo runs a custom L7 mitmproxy firewall and policies
  restrict paths (e.g. `api.github.com` only allows `/repos/anthropics/*`,
  everything else returns `blocked_path`). A 403 from a public API is far
  more likely a *local* firewall block than a remote-side issue. *How to
  apply* : `ls .devcontainer/firewall/policy.d/` then `cat <host>.yaml` to
  check `endpoints` + `blocked_paths` before retrying. If the target is
  legitimately needed, the right move is editing
  `policy.local.d/<host>.yaml` (gitignored) — not bypassing.

- **Always split commits by *target* : `templates/v2/*` and `.devcontainer/*`
  go in SEPARATE commits, never bundled.** *Why* : this repo's dual-edit
  pattern produces byte-identical changes in `templates/v2/` (the shipped
  template, picked up by future `install.sh` runs of adopting projects) and
  `.devcontainer/` (the dogfood mirror that this repo runs on itself).
  Bundling them obscures intent in `git log` and `git blame` — reviewers
  can't tell whether a hunk shipped to consumers or only patched the
  internal dogfood. Convention surfaced in commits `9fa7d25` / `40116ec`
  and `bb9c7fa` / `0363603`. *How to apply* : commit the `templates/v2/`
  side first (subject prefix `feat(template):`, `fix(template):`, etc.)
  with rollout-plan files riding along ; then commit the matching
  `.devcontainer/` mirror as `chore(dogfood): apply <change> to
  .devcontainer/`. If a single edit spans both — split the staging with
  `git add <path>` per file, never `git add -A`.

- **Devcontainer change order : edit in `.devcontainer/` → verify
  live → mirror byte-identical into `templates/v2/` → ship an
  `updates/<YYYYMMDD-HHMM>-<title>.md` entry (+ optional sibling
  `.patch`).** *Why* : `.devcontainer/` is the live testbed (daemon
  running, can iterate fast), so the change is shaped and validated
  there first. The `templates/v2/` mirror is what downstream FRESH
  installs pull from when a new project runs its first `./install.sh`
  — without the mirror, the change ships nowhere. The `updates/`
  entry is what downstream EXISTING installs pull from — without it,
  every consuming project has to hand-port the change. `install.sh`
  is never run as part of OUR workflow ; it's a downstream-only
  consumption surface. *How to apply* : (1) iterate inside
  `.devcontainer/`, with whatever ad-hoc verification fits (run the
  daemon, eyeball logs, unit tests). (2) Once happy, byte-copy each
  touched file into the matching `templates/v2/` path and `diff` the
  two to confirm zero drift. (3) Per the per-target commit-split rule
  above, commit `templates/v2/` first as `feat(template):` /
  `fix(template):` etc., then `.devcontainer/` as
  `chore(dogfood): apply <change> to .devcontainer/`. (4) Create
  `/workspace/updates/<YYYYMMDD-HHMM>-<title>.md` with a 5-backtick
  fenced `bash` block applying the change
  (inline `sed` for tiny diffs OR `git apply --check && git apply`
  on a sibling `.patch` for larger ones), then `git commit -m` with a
  pre-written message referencing the two upstream commit hashes.
  Skip only when the change is purely local dogfood scaffolding that
  intentionally does not ship — call that out explicitly in the
  session log.

- **Scripts you generate for the user to run on the *host* must use paths
  RELATIVE to the cwd, never `/workspace/...`.** *Why* : `/workspace` is a
  bind-mount point that only exists *inside* the container. On the host the
  same repo lives at an arbitrary path (e.g. `~/dev/myrepo/`,
  `/Users/x/Code/foo/`) that you don't know. Hardcoding `/workspace` makes
  the script fail with `cd: No such file or directory` on every host. *How
  to apply* : start with `#!/usr/bin/env bash` + `set -euo pipefail`, a
  header comment "run from repo root on host", and every path
  `./relative/...` or `$(pwd)/...`. Files dropped in `.tmp/foo/` on the
  host are then visible at `/workspace/.tmp/foo/` from inside the
  container via the bind mount.

- **Targeted updates ship as `updates/<YYYYMMDD-HHMM>-<title>/` folders
  containing exactly two files : `update.patch` + `update.md`.
  Downstream projects fetch them via sparse-checkout into
  `.tmp/devcontainer-updates/` — never into `.tmp/upgrade-v2/updates/`
  (that path was the old convention, retired June 2026).** *Why* :
  one folder per fix keeps `git log` / archival surgical, the
  sparse-checkout target is ~1 MB vs ~30 MB for the full-release
  clone used by the version-bump flow, and the `.tmp/devcontainer-updates/`
  prefix is short enough to type / paste without abbreviation. The
  bootstrap + per-update flow is documented in the "Targeted updates"
  section of [UPGRADE-v2.md](../UPGRADE-v2.md#targeted-updates-updatesname).
  *How to apply* : when shipping a new targeted fix, create
  `updates/<ts>-<title>/` with `update.patch` (the diff) and
  `update.md` (the recipe). Every path inside the recipe's bash
  blocks references `.tmp/devcontainer-updates/updates/<ts>-<title>/update.patch`.

- **`updates/*/update.md` bash blocks must be flat-pasteable in an
  interactive zsh — no `set -euo pipefail`, no `# …` step comments, no
  leading variable assignments, no multi-line `if/then/fi`.** *Why* : the
  user pastes these recipes line-by-line (or block-by-block) into the host
  terminal — never through `bash <<EOF`. `set -euo pipefail` in an
  interactive shell kills the whole session on the first unset var or
  non-zero exit ; `# 1.` / `# 2.` step headers and `PATCH=…` declarations
  either pollute history or fail to survive across multi-paste sessions.
  This rule was paid for by [updates/20260613-1934-notify-accents-state/update.md](../updates/20260613-1934-notify-accents-state/update.md)
  — the user had to manually rewrite it ; **DO NOT regenerate that shape**.
  *How to apply* : when authoring `updates/<YYYYMMDD-HHMM>-<title>/update.md`,
  model the Apply / Rollback blocks on the canonical
  [updates/20260613-0929-plus-button-chrome/update.md](../updates/20260613-0929-plus-button-chrome/update.md)
  — bare commands separated by blank lines, paths inlined (no `$VAR`,
  always `.tmp/devcontainer-updates/updates/<name>/update.patch` written
  in full), multi-line commands joined with `\` continuations,
  multi-statement conditionals folded onto one line (`if …; then …; fi`).
  If a recipe genuinely needs `set -e` semantics, ship a sibling `.sh`
  file and have the recipe call `bash ./that.sh` — never inline the guards
  in the `.md`.

- **Commit messages stay short and self-contained — never append
  `— apply updates/<ts>-<title>` or any other rollout/tracker suffix.**
  *Why* : `git log` / `git blame` / PR diffs are read without the
  rollout doc open. A suffix like `— apply updates/20260613-1934-notify-accents-state`
  bloats the subject past the 50-char readable budget, decays the
  moment the `updates/` entry is archived, and adds zero information
  the diff doesn't already carry. The subject must describe **the
  change itself** (`fix(notify): accents decode + state payload`),
  not the delivery vehicle. Reinforces §10 of
  [CLAUDE-dev.md](claude/CLAUDE-dev.md). *How to apply* : when
  writing a `git commit -m` line inside an `updates/*/update.md`
  recipe, in a session prompt, or anywhere else — stop at the change
  description ; never tack on `— apply updates/…`, `— rollout step N`,
  `— plan abc123`, etc. If a reviewer needs the rollout context, the
  PR description / commit body is where it goes, not the subject.

- **Before searching the web for tooling docs, check
  `.devcontainer/knowledge/<tool>.md` first.** *Why* : this repo ships
  cheat-sheets for every dev-tool baked into the base image — `wtf.md`,
  `firewall.md`, `docker-base-image.md`, `ollama-local.md`,
  `extension-points.md`. They're compiled from the tool's own source and
  refreshed when the base image bumps. Reaching for WebFetch / gh api
  instead means (a) burning permission prompts on `Bash(gh:*)` /
  `Bash(wtf:*)`, (b) chasing 404s on stale doc-site URLs, and (c)
  duplicating the exact schema already spelled out in the knowledge file.
  Real hit : spent ~5 tool calls fetching wtfcmd docs online for the
  `is_array` variadic + `cwd:` + `--` passthrough syntax — every field
  was in `knowledge/wtf.md` already. *How to apply* : first move on any
  « how does `<X>` work » question is `ls .devcontainer/knowledge/` +
  `Read` the matching file. The « Canonical links » footer at the end
  of each cheat-sheet is the escape hatch when the local doc feels
  stale ; the doc itself is the primary source.
