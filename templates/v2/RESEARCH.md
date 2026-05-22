# RESEARCH — scoped scope-expansion via research bundles

> The main devcontainer runs the **Niveau 1 strict** firewall : only
> `api.anthropic.com` (+ minimal telemetry + anthropics git smart-pack) is
> reachable for POST, and the GET allowlist is the Claude-only baseline (17
> hosts). When a task genuinely needs to talk to a third-party API or fetch
> docs outside that baseline, the **only** sanctioned mechanism is to spawn a
> **research devcontainer** in a sibling folder with its own scoped firewall.
>
> No temporary grants. No `domains.local.txt` edit-in-the-main shortcuts. The
> research bundle is the one and only door.

For the higher-level architecture (3-container model) see
[docs/ARCHITECTURE.md](../plans/devcontainer-v2/phase3-rollout/docs/ARCHITECTURE.md).
For the cycle diagram see
[docs/WORKFLOWS.md](../plans/devcontainer-v2/phase3-rollout/docs/WORKFLOWS.md#research-workflow--élargissement-scope-contrôlé).

## End-to-end cycle

```
main container         host                       research container
─────────────────      ───────────────────        ────────────────────
/prepare-research      (review bundle)            (Claude works in
  → bundle written     cp -r → sibling             /workspace/result/)
                       code <sibling>/
                       (fill .env.local)
                                                  → result/OUTPUT.md, …
                       bring-back-result          ←
                       (archive + promote)
                       research-cleanup --apply
```

### 1 — Generate the bundle (main container)

```
/prepare-research <description>
/prepare-research <template> <description>
```

The skill (committed under `.devcontainer/skills/prepare-research/`) writes a
self-contained bundle to
`.devcontainer/research-bundles/<bundle-id>/`. The `bundle-id` is
`<main-dc-project>-research-<task-slug>` — e.g.
`dc-project-research-stripe-payment`.

Pick a template hint or let the skill auto-detect :

| Template | Use case |
|---|---|
| `new-api-integration` | Integrate a third-party API (Stripe, Linear, Resend, …) — POST allowed on the API host |
| `doc-research` | Read-only doc / source exploration — GET only, no secrets |
| `package-evaluation` | Evaluate an npm / composer / pip / cargo / go package before adoption |

See [skills/prepare-research/templates/](skills/prepare-research/templates/)
for the YAML source.

### 2 — Review the bundle (host)

```
ls .devcontainer/research-bundles/<bundle-id>/
cat .devcontainer/research-bundles/<bundle-id>/START.md
cat .devcontainer/research-bundles/<bundle-id>/INSTRUCTIONS.md
cat .devcontainer/research-bundles/<bundle-id>/.devcontainer/firewall/domains.local.txt
ls  .devcontainer/research-bundles/<bundle-id>/.devcontainer/firewall/policy.local.d/
```

Check the proposed allowlist additions match the task : no overly broad
wildcards, no third-party POST you didn't ask for, no secret leaked into
`secrets.env.template` (every value must be `REPLACE_ME`).

### 3 — Materialise as a sibling (host, from main project root)

```
cp -r .devcontainer/research-bundles/<bundle-id>/ ../<bundle-id>/
```

That's the entire host import step. The bundle is already a self-contained
`.devcontainer/` + workspace files at top-level paths preserved — no parsing,
no merging, no rewrite.

If `secrets.env.template` is non-empty :

```
cp ../<bundle-id>/.devcontainer/.env ../<bundle-id>/.devcontainer/.env.local
$EDITOR ../<bundle-id>/.devcontainer/.env.local       # fill REPLACE_ME
# the research container reads .env.local with priority over .env
```

### 4 — Open the research container

```
code ../<bundle-id>/
```

VS Code prompts "Reopen in Container" → accept. The research container boots
with :

- `DC_PROJECT=<bundle-id>` → isolated `bash-history`, `claude-config`,
  `mitmproxy` CA volumes (no cross-pollution with the main).
- `CLAUDE_CREDS_VOLUME=<main's claude-creds volume>` → Claude is **already
  authenticated** in the research container ; no re-login.
- The skill-written `firewall/domains.local.txt` + `firewall/policy.local.d/`
  on top of the verbatim baseline → expanded allowlist, fail-closed otherwise.
- `FIREWALL_MODE=strict` (the whole point of the research is scoped strict ;
  don't drop to `basic` unless you genuinely understand the risk).

The first VS Code prompt after attach should be the copy-paste prompt printed
in `START.md` — it tells Claude to read `/workspace/INSTRUCTIONS.md` and start
working in `/workspace/result/`. The prompt is **not** auto-loaded ; you must
paste it.

### 5 — Bring back the result (host-only, from main project root)

> **Host-only.** The helper reads `../<bundle-id>/result/.` (a sibling of the
> main project root), which only exists on the host filesystem. The helper
> refuses (exit 2) if invoked inside the devcontainer.

```
.devcontainer/host-helpers/bring-back-result <bundle-id>
```

Archives `../<bundle-id>/result/.` into
`.devcontainer/research-bundles/<bundle-id>/result/` — a gitignored snapshot
inside the main project for later review. Surconfirme before copying.

To also promote selected files to their final destination :

```
.devcontainer/host-helpers/bring-back-result <bundle-id> <dest-dir>
```

The second arg is the target path inside the main project (e.g.
`docs/stripe/`, `src/integrations/stripe/`, `.devcontainer/wtfcmd-research/`).
The helper surconfirme each copy step independently — answer `no` to the
promote prompt to keep just the archive.

The sibling folder is **not** deleted by this helper. Cleanup happens
separately via `research-cleanup` (next step) so you can re-run the research
container if needed before dropping it.

### 6 — Cleanup the sibling (host-only, from main project root)

> **Host-only.** Same constraint as bring-back-result : scans `../*-research-*`
> on the host filesystem, refuses (exit 2) inside the devcontainer.

```
.devcontainer/host-helpers/research-cleanup
```

Dry-run by default — lists sibling folders matching
`../<main-dc-project>-research-*/` older than 7 days (override with
`RESEARCH_CLEANUP_DAYS=N`).

```
.devcontainer/host-helpers/research-cleanup --apply
```

Surconfirme then deletes the listed folders. Print the matching Docker volume
prune command at the end :

```
docker volume ls --format '{{.Name}}' | grep -- '-research-' | xargs -r docker volume rm
```

The helper is **not** hooked to `post-start.sh` (unlike `watch-log-cleanup`).
Research siblings can represent days of work — automatic deletion would be
hostile. Manual invocation only, dry-run is the safe default.

Single-shot manual cleanup also works :

```
rm -rf ../<bundle-id>/
docker volume ls | grep -- '-research-<task-slug>'      # confirm targets
docker volume rm <each-volume>                          # explicit drops
```

Do **not** drop the main's `claude-creds` volume — it's shared with the
research and the main relies on it for Anthropic auth.

## Bundle anatomy (reference)

Authoritative layout :
[skills/prepare-research/prepare-research.skill.md](skills/prepare-research/prepare-research.skill.md)
§ "Bundle layout".

TL;DR :

```
<bundle-id>/
├── START.md                       host entry-point + copy-paste prompt
├── INSTRUCTIONS.md                task brief read by Claude
├── secrets.env.template           REPLACE_ME values, never real keys
├── .claude/settings.json          pre-allowed read-only Bash + WebFetch
├── result/.keep                   sentinel — Claude's output lands here
├── .devcontainer/                 verbatim copy of main's .devcontainer/
│   ├── Dockerfile / devcontainer.json / docker-compose.yml / …
│   ├── .env                       DC_PROJECT=<bundle-id>, CLAUDE_CREDS_VOLUME=…
│   └── firewall/
│       ├── domains.txt / policy.d/         verbatim (Claude-only baseline)
│       ├── domains.local.txt               NEW (research allowlist additions)
│       └── policy.local.d/<host>.yaml      NEW (per-host advanced rules)
└── <workspace files>              paths preserved (src/…/Stripe.ts, package.json, …)
```

## Security model

- **Bundle is reviewable** — every file is plain text, sitting in
  `.devcontainer/research-bundles/<bundle-id>/`. Review before `cp -r`. The
  diff vs the main `.devcontainer/` is the
  `firewall/domains.local.txt` + `firewall/policy.local.d/<host>.yaml` overlay.
- **Committed baseline never rewritten** — `firewall/domains.txt` and
  `firewall/policy.d/*.yaml` in the bundle are copied verbatim from the main.
  Research additions go through the local overlay (gitignored in the main, but
  just a plain file once inside the bundle).
- **Secrets are template-only** — `secrets.env.template` ships `REPLACE_ME`
  values. The skill validates this post-write and refuses to ship a bundle
  with anything that looks like a real key (≥ 20 alphanumeric chars).
- **Volume isolation** — `DC_PROJECT=<bundle-id>` gives the research its own
  `bash-history`, `claude-config`, `mitmproxy` CA volumes. The main's
  `claude-config` (skills, memory) is **not** visible from the research, so
  `/prepare-pr`, `/prepare-research`, `/watch-log` are unavailable inside the
  research (don't try to invoke them).
- **`claude-creds` is shared** — single Anthropic credential across main +
  research(es). Convenient (no re-login) but it does mean any research has the
  same Anthropic API access as the main. This is acceptable because the
  firewall (not the credential) is the access boundary.
- **Fail-closed** — the firewall mitmproxy addon answers `503` on unknown
  hosts / paths / methods / body sizes. If a research task tries to reach a
  domain that wasn't allowlisted in `domains.local.txt`, the request is
  blocked even though Claude is running.

## Cleanup conventions

| What | Where | Tool | Default |
|---|---|---|---|
| Sibling folder `../<bundle-id>/` | host | `research-cleanup` | dry-run, `--apply` to delete |
| Bundle source `.devcontainer/research-bundles/<bundle-id>/` | host | manual `rm -rf` | gitignored ; safe to delete after archive |
| Docker volumes `*-research-<task-slug>*` | host | `docker volume rm` (or `docker volume prune`) | manual ; helper prints the command |
| `claude-creds-shared-<…>` volume | host | **never delete** | shared with the main |

Recommended frequency : run `research-cleanup` (dry-run) once a week to see
what's lingering. Promote / archive what you want to keep via
`bring-back-result`, then `--apply`.

## Troubleshooting

### `docker compose up` fails on `external: true` volume

The bundle's `.env` references `CLAUDE_CREDS_VOLUME=<main's volume name>`. If
the main has never been initialised (so the `claude-creds-shared-*` volume
doesn't exist yet), the research compose can't mount it.

Fix : boot the main once first (`initialize.sh` creates the volume on first
container build). Or hand-create it : `docker volume create
<volume-name>`.

### Skill output mentions `<bundle_id>` or `{{var}}` literally

That's a substitution bug — the skill should never ship template placeholders.
Re-invoke `/prepare-research` and report the bundle name ; the skill body has
a sanity grep at the end that should catch it.

### `bring-back-result` says "bundle source missing"

The bundle source `.devcontainer/research-bundles/<bundle-id>/` must still
exist in the main project for the archive snapshot. If you deleted it (e.g.
re-cloned the repo), recreate the directory or just copy the result manually :

```
cp -r ../<bundle-id>/result/. ./<dest>/
```

### Research container has no `/prepare-research`, `/prepare-pr`, …

By design — the research has its own `claude-config` volume (per the
`DC_PROJECT` isolation), so the skills synced by `sync-skills.sh` are
installed in the main's volume, not the research's. The first
`sync-skills.sh` run inside the research will install them, but you typically
don't want them there (avoid recursive `/prepare-research` from inside a
research container).

### A research session needs more domains

Stop the research container. Edit `.devcontainer/firewall/domains.local.txt`
(or `policy.local.d/<host>.yaml`) inside the sibling folder, then VS Code
"Rebuild Container". The firewall re-compiles at boot. If you find yourself
doing this often, the research scope was probably wrong — kill it and
re-generate a bundle with the broader scope from the start.

### Sibling folder name mismatch with bundle source

The helper expects `<bundle-id>` to be the same for both
`../<bundle-id>/` and `.devcontainer/research-bundles/<bundle-id>/`. If you
renamed one, rename the other or pass the actual sibling name. The skill
guarantees they match at generation time — only manual renames break the
invariant.
