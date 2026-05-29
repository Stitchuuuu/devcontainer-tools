# LESSONS — project-wide patterns (committed)

> Sister files : `LESSONS.local.md` (gitignored) for personal /
> not-yet-generalisable lessons. Cross-project preferences live in
> `~/.claude/memory/` (auto-memory).

<!-- Entries : one bullet per lesson. Rule first, then *Why* and
     *How to apply* on the same or following line. -->

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
