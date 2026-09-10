# v3 project `.devcontainer/` — copy-paste template

The thin project layer that consumes the published base image
`ghcr.io/meitogi/devcontainer-sandbox:<base>-cc<cc>`. Everything heavy
(toolchain, Claude Code patché, firewall machinery, lifecycle hooks) lives
in the image ; this folder owns only what is project-specific : the
firewall allowlist, the config, and the extension points.

This is the hand-assembled precursor of what `devc init` will scaffold
(session 5). Validated shape : the fw-bake Dockerfile and compose args
mirror the `dogfood-switchover` branch, proven by a 19/19 host smoke.

## Install

1. Copy this folder into your project as `.devcontainer/` (drop this
   README, or keep it — it is inert).
2. Replace the two placeholders everywhere (project id = lowercase slug,
   `[a-z0-9-]`) :

   ```bash
   cd .devcontainer
   grep -rl '{{PROJECT_' . --exclude=README.md | xargs sed -i '' -e 's/{{PROJECT_ID}}/my-project/g' -e 's/{{PROJECT_DISPLAY_NAME}}/My Project/g'
   ```

   (GNU sed : `sed -i` sans `''`.)
3. `cp .env.example .env`, then set at least `DC_PROJECT=my-project`.
   `BASE_IMAGE` is optional — the compose default pins the current
   published tag.
4. The Claude credentials volume : either let `initialize.sh` create the
   per-project one on first run, or point `CLAUDE_CREDS_VOLUME` in `.env`
   at an existing shared volume.
5. VS Code → "Reopen in Container". First boot pulls the base image
   (~2-3 GB), bakes your `firewall/` allowlist into the project layer, and
   runs the lifecycle through `devc-hook` (image-side fragments + your
   overlays under `hooks/`).

## What's in here

| Entry | Role |
|---|---|
| `devcontainer.json` | thin config — `devc-hook` lifecycle, no extension pin (the image bakes the patched one), `customizations.stitchu-devc.disabledHooks` |
| `Dockerfile` | fw-bake stage + `FROM ${BASE_IMAGE}` ; add project `RUN`s in the final stage (stacks : see the base repo's `stacks/*.md`) |
| `docker-compose.yml` | `BASE_IMAGE` + `FIREWALL_ALLOW_LOCAL_AT_REBUILD` build args, volumes, caps |
| `initialize.sh` | host-side pre-container step (creds volume, `.env` sync, prompts). Patched : no `Dockerfile.base` → no local base build. Will be replaced by `npx @stitchu/devcontainer-cli initialize` once published |
| `firewall/` | YOUR additions on top of the base-image allowlist (Claude Code / VS Code / sandbox tooling ship in the image — a fresh project needs nothing here) : `domains.txt` (curated, committed), `domains.d/` (auto-extracts), `policy.d/` (L7), `.local` variants gitignored — never baked unless `FIREWALL_ALLOW_LOCAL_AT_REBUILD=1`. Override a base host with `!disable` (+ optional redeclare) in `domains.txt` |
| `hooks/{on-create,post-create,post-start}.d/` | project overlay fragments — same filename as an image fragment masks it, unique filename adds |
| `.env.example` | documented knobs ; copy to `.env` (gitignored) |

## Known gaps (pre-session-5)

- `claude/CLAUDE-*.md` seeds are not scaffolded yet — the image's
  `10-symlink-claude-mode` post-create fragment will warn (non-fatal).
- Root `.gitignore` append and `LESSONS.md` symlink are manual for now.
- Requires the image to be published and public (TEST-PLAN-4 S3-S4).
