---
description: Spawn a scoped research devcontainer with an expanded firewall allowlist. Use when the user asks for web research, up-to-date docs, library/package evaluation, third-party API integration, or any query needing sources outside the strict Niveau 1 baseline (17 hosts). The main firewall blocks most outbound — don't WebFetch/WebSearch silently and fail; propose this skill or a targeted firewall/domains.local.txt addition.
argument-hint: "<description> | <template> <description>"
---

# /prepare-research — generate a self-contained research bundle

The main devcontainer runs the Level 1 strict firewall : 17 hosts allowlisted
(Claude-only baseline), all third-party POST blocked. The **only** sanctioned
way to extend the network scope (new API integration, doc research, package
evaluation) is to spawn a **research devcontainer** in a sibling folder with
its own scoped firewall. No temporary grants in the main.

This skill produces a self-contained **research bundle** under
`/workspace/.devcontainer/research-bundles/<bundle-id>/`. The bundle is
designed so a single host-side `cp -r ../<bundle-id>/` is enough to materialise
a ready-to-open VS Code project that spawns the research container with the
expanded allowlist. (`<bundle-id>` = `<main-dc-project>-research-<task-slug>` —
see §"Bundle identity" below.)

## When to use

- The user asks for **web research, up-to-date docs, or sources** that go
  beyond the 17-host Niveau 1 baseline (e.g. "recherche-moi X", "explore
  Workflows", "read the latest about Y"). The main `WebFetch`/`WebSearch`
  will silently fail on non-allowlisted hosts in both `strict` and `basic`
  modes — don't try and fail, propose this skill instead.
- The user wants to **read a third-party library / API** whose domain is not
  in `firewall/domains.txt`.
- The user says : "prépare une recherche", "intègre Stripe", "explore l'API
  Linear", "évalue le package X", or anything that requires POST / GET on a
  domain outside the baseline.
- The user wants a clean, isolated workspace to experiment with a third-party
  API or library without touching the main project state.

**Pivot vs targeted allowlist** : if the user only needs 1–2 read-only
domains for a single-session lookup, propose adding them to
`firewall/domains.local.txt` (+ `policy.local.d/<host>.yaml` if POST) and
Rebuild Container. Reserve `/prepare-research` for multi-host, multi-session,
or any POST to a third-party — it's the sanctioned, audited path and avoids
polluting the main `domains.local.txt`. When in doubt, default to
`/prepare-research`.

If the user only needs to read more docs from already-allowlisted hosts, do
NOT spawn a research bundle — answer directly. The bundle exists for genuine
scope extension.

## Bundle identity — `bundle_id`

Every research bundle is identified by `bundle_id` = `<main-dc-project>-research-<task-slug>`.

- `<main-dc-project>` is the main container's `DC_PROJECT` env var (default
  `dc-project`).
- `<task-slug>` is the kebab-case identifier derived from the user's
  description (see §1b).

Examples : `dc-project-research-wtfcmd`,
`dc-project-research-stripe-payment`.

The `bundle_id` is used **everywhere** : the bundle directory name, the host
sibling folder name, and the research container's `DC_PROJECT`. This ensures :
- No collision when several main projects on the same host spawn research
  bundles with the same `<task-slug>` (e.g. two projects exploring `zod`).
- Volume names at `docker volume ls` show clearly which main spawned which
  research (e.g. `claude-code-bashhistory-dc-project-research-wtfcmd`).
- `claude-creds` remains shared with the main (resolved separately, not via
  `bundle_id`).

## Bundle layout

```
/workspace/.devcontainer/research-bundles/<bundle-id>/
├── .devcontainer/                     ← verbatim copy of /workspace/.devcontainer/
│   ├── Dockerfile / devcontainer.json / docker-compose.yml
│   ├── post-create.sh / post-start.sh / shell-init.sh / init-firewall.sh / …
│   ├── claude/ skills/ host-helpers/ tests/
│   ├── pending/.keep   pr-drafts/.keep
│   ├── .env                           ← NEW (you generate this) :
│   │                                     DC_PROJECT=<bundle-id>
│   │                                     CLAUDE_CREDS_VOLUME=<main creds volume>
│   ├── firewall/
│   │   ├── domains.txt                verbatim (Claude-only baseline)
│   │   ├── policy.d/                  verbatim (committed *.yaml unchanged)
│   │   ├── domains.local.txt          ← NEW (you generate this) :
│   │   │                                 header `# Research bundle: <task>`
│   │   │                                 + research-specific allowlist additions
│   │   ├── policy.local.d/            ← NEW (you generate this) :
│   │   │   └── <new-host>.yaml        advanced rules for the added hosts
│   │   ├── domains.local.txt.example  verbatim
│   │   ├── policy.local.d.example/    verbatim
│   │   └── …                          (addons/ tests/ compile-policy.py …)
│   └── …
├── START.md                           ← user entry-point : steps to open in
│                                        VS Code + the copy-paste prompt for
│                                        Claude (the prompt is NOT auto-loaded
│                                        when VS Code attaches the container,
│                                        so the user MUST paste it manually)
├── INSTRUCTIONS.md                    brief read by Claude when the prompt
│                                      tells it to read /workspace/INSTRUCTIONS.md
├── .claude/
│   └── settings.json                  permissions.allow for read-only Bash +
│                                      WebFetch / WebSearch (curl, grep, find,
│                                      ls, cat, jq, git log/diff/show, …).
│                                      Firewall remains the enforcement layer ;
│                                      these perms just suppress prompts.
├── secrets.env.template               REPLACE_ME values (user fills .env.local)
├── result/.keep                       sentinel — Claude's output lands here
│                                      (OUTPUT.md, PROGRESS.md, notes/)
└── <workspace files at top-level>     paths preserved from main
    ├── src/payment/Stripe.ts          ex for stripe-integration
    ├── package.json
    └── tsconfig.json
```

`<bundle-id>` everywhere in the layout above is the full concat — e.g.
`dc-project-research-wtfcmd`.

The committed firewall files (`domains.txt`, `policy.d/`) are NEVER rewritten.
All research additions go through the local overlay (`domains.local.txt`,
`policy.local.d/`) — that overlay is gitignored in the main repo but is just a
plain file once copied into the bundle. The compile step at research boot
(`compile-policy.py`) merges baseline + overlay automatically.

The `claude-creds` Docker volume is **shared** with the main so Claude is
already authenticated when the research container spawns. Other volumes
(`bash-history`, `claude-config`, `mitmproxy` CA) stay isolated via the
research-specific `DC_PROJECT` name.

## Process

### 1. Parse the invocation

The skill accepts **two invocation shapes** — detect which one applies before
asking the user anything.

Known template names (in `/workspace/.devcontainer/skills/prepare-research/templates/`) :
`new-api-integration`, `doc-research`, `package-evaluation`.

**Shape A — free-form description** : `/prepare-research <description>`
  Example : `/prepare-research explore wtfcmd to learn how to create a wtf command`
  → Auto-detect template from keywords (see §1a). Auto-derive `task-slug` from the
  description (see §1b).

**Shape B — explicit template + description** : `/prepare-research <template> <description>`
  First whitespace-separated token matches a known template name → use that
  template ; the rest is the description. Auto-derive `task-slug` from description.
  Example : `/prepare-research doc-research wtfcmd usage`

If no description is given at all (`/prepare-research` alone), ask the user
what they want to research.

#### 1a. Template auto-detection from description (Shape A)

Score each template against the description (case-insensitive, accent-insensitive
keyword match) :

| Template | Trigger keywords (FR + EN) |
|---|---|
| `new-api-integration` | intègre, intégrer, integration, integrate, API, endpoint, webhook, sandbox, third-party, POST, REST, GraphQL, SDK, Stripe, Linear, Resend, Twilio, SendGrid |
| `doc-research` | doc, documentation, recherche, research, explore, explorer, lire, read, synthétiser, synthesize, understand, comprendre, learn, apprendre, tutorial, guide, how-to, RTFM |
| `package-evaluation` | évalue, évaluer, evaluate, eval, package, lib, library, npm, composer, pip, cargo, crate, gem, adopter, adoption, alternative, comparison, benchmark, fit |

Pick the highest score. If two templates are tied OR no template scores ≥ 1, use
`AskUserQuestion` with the 3 templates as options (header `Template`, question
"Which research type fits best?", each option lists template name
+ one-line use case). DO NOT silently pick a template when ambiguous.

#### 1b. Task-slug derivation from description

Extract the most salient noun-phrase from the description, kebab-case it, and
keep it ≤ 30 chars. Strip stopwords (a, the, of, an, for, in, on, with, to, le,
la, les, de, du, des, pour, sur). Drop the trigger keywords already used to pick
the template (we don't want "explore wtfcmd" → `explore-wtfcmd`, prefer just
`wtfcmd`).

Examples :
- "explore wtfcmd to learn how to create a wtf command" → `wtfcmd`
- "intègre Stripe dans le module payment" → `stripe-payment` (keep the scope hint)
- "évalue le package zod pour validation runtime" → `zod-validation`
- "explore Linear API for ticket sync" → `linear-ticket-sync`

If the derived slug is too generic (≤ 3 chars, or matches a single common verb),
ask the user for an explicit slug before continuing.

Always confirm the inferred `task-slug` + `template` + computed `bundle_id`
with the user **before** proceeding — print one short line :

```
→ Detected: template=<name>, task-slug=<slug>, bundle-id=<main>-research-<slug>.
  Continue? (yes / no / change)
```

Wait for explicit confirmation. If the user says "change", ask for the slug
and/or template. This single confirmation is mandatory ; without it, a typo in
the description leads to a misnamed bundle that the user has to delete and redo.

#### 1c. Validate `task-slug` and compute `bundle_id`

- `task-slug` must be lower-case kebab : `[a-z0-9-]+`. Reject if contains `/`,
  `..`, uppercase, or spaces.
- Resolve `main_dc_project` :
  ```bash
  main_dc_project="${DC_PROJECT:-}"
  if [ -z "$main_dc_project" ]; then
    main_dc_project=$(grep -E '^DC_PROJECT=' /workspace/.devcontainer/.env 2>/dev/null | cut -d= -f2 | tr -d '"' || true)
  fi
  main_dc_project="${main_dc_project:-dc-project}"
  ```
- Compute `bundle_id="${main_dc_project}-research-${task_slug}"`.
- `/workspace/.devcontainer/research-bundles/<bundle_id>/` MUST NOT already
  exist. If it does, append `-v2`, `-v3`, … to `task_slug` (and recompute
  `bundle_id`) and propose the new identifier for user confirmation.

### 2. Collect template inputs

Read the template YAML (`/workspace/.devcontainer/skills/prepare-research/templates/<template>.yaml`)
and ask the user for each `prompts_for_user` entry. Pre-fill from the description
when obvious :

- For `doc-research`, if the description mentions specific domains (`wtfcmd.dev`,
  `github.com/blunt1337/wtfcmd`), pre-fill `doc_domains` with those.
- For `new-api-integration`, if a service name is in the description ("intègre
  Stripe…"), pre-fill `service_name=Stripe` and infer `api_domain=api.stripe.com`
  (still confirm with user — domain inference can be wrong).
- For `package-evaluation`, if a package name is in the description ("évalue
  zod"), pre-fill `package_name=zod`.

For each `prompts_for_user` entry, ask only if the value wasn't pre-filled.
Display defaults clearly (`[default: GET,POST]`) and accept empty input to take
the default.

### 3. Generate the bundle

All paths below are absolute. `BUNDLE=/workspace/.devcontainer/research-bundles/<bundle_id>`
(remember : `bundle_id = ${main_dc_project}-research-${task_slug}`).

#### 3a. Create the skeleton

```bash
mkdir -p "$BUNDLE/.devcontainer/firewall/policy.local.d"
mkdir -p "$BUNDLE/result"
touch "$BUNDLE/result/.keep"
```

#### 3b. Copy main `.devcontainer/` verbatim, with mandatory excludes

`rsync` is installed in the base image (added in the Dockerfile alongside
`python3-yaml`). Use it with explicit excludes — the excludes prevent leaking
runtime artefacts and the current dev's personal overrides.

```bash
rsync -a \
  --exclude='research-bundles' \
  --exclude='pr-drafts/*' --include='pr-drafts/.keep' \
  --exclude='pending/*'   --include='pending/.keep' \
  --exclude='tests/diagnose.log' \
  --exclude='tests/diag-a2-*.log' \
  --exclude='firewall/addons/__pycache__' \
  --exclude='firewall/domains.local.txt' \
  --exclude='firewall/policy.local.d/*' \
  --exclude='.env' \
  --exclude='.configured-*' \
  /workspace/.devcontainer/ "$BUNDLE/.devcontainer/"
```

Why each exclude :
- `research-bundles` — avoids recursive bundle-in-bundle.
- `pr-drafts/*`, `pending/*` — runtime user artefacts from /watch-log
  and /prepare-pr ; `.keep` is preserved so the directory still exists.
- `tests/diagnose.log`, `tests/diag-a2-*.log` — test output.
- `firewall/addons/__pycache__` — Python bytecode.
- `firewall/domains.local.txt`, `firewall/policy.local.d/*` — current
  dev's personal overrides ; the skill writes its own.
- `.env`, `.configured-*` — local state files of the main.

If `rsync` is unexpectedly missing (older container that predates the dep
addition), abort and tell the user to **rebuild the container** so the new
Dockerfile pulls in `rsync`.

#### 3c. Resolve the main's `claude-creds` volume name

`main_dc_project` is already resolved in §1c. Compute the creds volume :

```bash
creds="${CLAUDE_CREDS_VOLUME:-claude-creds-${main_dc_project}}"
```

This points the bundle's research container at the main's existing
`claude-creds` Docker volume (`external: true`). Without sharing, the
research container would launch with no Anthropic token.

#### 3d. Write `bundle/.devcontainer/.env`

```
# Generated by /prepare-research for bundle: <bundle_id>
# DO NOT commit this file — it is gitignored at workspace root.

# Bundle-scoped isolation : bash-history, claude-config, mitmproxy CA each
# get their own volume named claude-code-<kind>-<bundle_id>. The bundle_id
# concats the main DC_PROJECT and the task slug so volumes never collide
# across main projects.
DC_PROJECT=<bundle_id>

# Share claude-creds with the main so Claude is already authenticated.
# Volume name resolved from the main's $CLAUDE_CREDS_VOLUME / $DC_PROJECT.
CLAUDE_CREDS_VOLUME=<resolved name from step 3c>

# Firewall mode default (strict). Change to `basic` only with explicit user
# request — the whole point of research is scoped strict.
FIREWALL_MODE=strict
```

#### 3e. Write `bundle/.devcontainer/firewall/domains.local.txt`

Header :

```
# Research bundle: <bundle_id>
# Generated: <ISO 8601 date, e.g. 2026-05-19T14:32:00Z>
# Additions to the main baseline (committed domains.txt is unchanged).
# Syntax: see /workspace/.devcontainer/firewall/domains.local.txt.example
#         and /workspace/.devcontainer/firewall/POLICY.md for the 5 forms.
```

Body : research-specific entries using the A1.1 extended syntax. Examples :

```
# GET only (default — no method prefix)
docs.stripe.com

# Specific methods on a host
[GET,POST] api.stripe.com

# Disable a baseline entry (rare — needed if the baseline blocks something
# you need to override more loosely)
!disable some-baseline-host.example
```

Constraints :
- **Refuse wildcard hosts** like `*.com` or `*` — they defeat the firewall.
  Sub-wildcards on a specific public suffix (`*.stripe.com`) are OK if
  justified.
- Method list MUST be uppercase comma-separated, no spaces inside brackets.

#### 3f. Write `bundle/.devcontainer/firewall/policy.local.d/<host>.yaml` per new host

Only emit when the user needs advanced rules (max body size, schema for
specific endpoints). Mirror the format of `/workspace/.devcontainer/firewall/policy.d/*.yaml`.

Minimal template for a new host needing POST :

```yaml
# Generated by /prepare-research for bundle: <bundle_id>
endpoints:
  - methods: [GET, POST]
    max_body_kb: 100   # adjust upward if the API needs larger payloads (max ~1024)
```

If the user provided endpoint paths (e.g. `/v1/charges`), make them explicit :

```yaml
endpoints:
  - path: /v1/charges
    methods: [POST]
    max_body_kb: 100
  - path: /v1/charges/*
    methods: [GET]
```

Every `policy.local.d/<host>.yaml` MUST specify `max_body_kb` explicitly.

After writing, sanity-check syntax :

```bash
python3 /workspace/.devcontainer/firewall/compile-policy.py \
  --parse-only "$BUNDLE/.devcontainer/firewall/domains.local.txt"
```

If non-zero exit, **abort and tell the user** which line broke.

#### 3g. Write `bundle/INSTRUCTIONS.md`

Template :

```markdown
# Bundle: <bundle_id>  (task: <task-slug>)

## Goals
- <objective 1>
- <objective 2>

## Constraints
- Scope to <relevant directories>
- Output proposal in /workspace/result/ (this directory will be copied back
  to the main project by the user after the session ends).
- Test with sandbox/test credentials only — NEVER production.
- This research container has its own `claude-config` volume : project skills
  (`/scan-deps`, `/prepare-pr`, …) are NOT available here. Do not invoke them.

## Success criteria
- <criterion 1>
- <criterion 2>

## Multi-session conventions
A research bundle may span several Claude sessions (the firewall scope and
the workspace persist via Docker volumes ; only the conversation context is
lost between sessions). Conventions :

- `/workspace/result/PROGRESS.md` — running index of what's done and what
  remains. Claude reads it at the start of every session (the START.md prompt
  tells it to) and appends after each session.
- `/workspace/result/OUTPUT.md` — the final synthesis / proposal / verdict.
  Drafted incrementally across sessions ; may be partial until the last one.
- `/workspace/result/notes/<topic>.md` — intermediate findings (optional).

If you (Claude) believe the task fits a single session, just write OUTPUT.md
straight away and leave PROGRESS.md as a one-liner (`done in 1 session`).

## Back-copy hint (last message of the session)
At the end of the work, propose the following two-step procedure for the
user, to run on their host **from the main project root** :

1. Archive the result back into the bundle source (snapshot of the session
   alongside the original bundle config — gitignored, local-only) :

       cp -r ../<bundle_id>/result/. ./.devcontainer/research-bundles/<bundle_id>/result/

2. Then promote specific deliverables to their final destination based on
   what was produced (code → `src/`, doc → `docs/`, etc.). Example :

       cp ../<bundle_id>/result/<file> ./<final-path>/
```

> 🚨 **When generating this file, replace every `<bundle_id>` with the
> actual computed value** (e.g. `dc-project-research-wtfcmd`).
> The literal string `<bundle_id>` MUST NOT appear in the generated
> `INSTRUCTIONS.md` — it would leave the user with un-executable
> placeholder paths.

#### 3h. Write `bundle/START.md`

The user opens the bundle in VS Code → the research container spawns, but
Claude in that container starts **without any initial prompt** (the harness
doesn't auto-load a system prompt from the workspace). The user MUST copy-paste
something to kick off the session. `START.md` is the entry point that ships
that prompt.

Template :

```markdown
# Research session — START HERE

Welcome to research bundle `<bundle_id>`. This is your kick-off page.

## 1. (Once, on the host) Fill the secrets

Before opening this folder in VS Code, on your host :

    cp secrets.env.template .devcontainer/.env.local
    $EDITOR .devcontainer/.env.local

Replace every `REPLACE_ME` with a sandbox / test value. The file is gitignored.
Skip this step if `secrets.env.template` contains only comments (no secrets
required for this research type).

## 2. Open this folder in VS Code → "Reopen in Container"

The research container spawns with the scoped firewall and the main's
`claude-creds` volume mounted, so Claude is already authenticated to
Anthropic — no re-login needed.

## 3. Copy-paste this prompt into Claude

Once Claude is ready in the research container, paste the prompt block
below exactly :

````
<initial_prompt rendered from the template + placeholders substituted>
````

That prompt tells Claude to read `INSTRUCTIONS.md`, resume from
`/workspace/result/PROGRESS.md` if it exists, and update `OUTPUT.md` +
`PROGRESS.md` as it progresses.

## Multi-session research

If the work doesn't fit one session, re-open this folder later and paste the
same prompt again. Claude will pick up from `PROGRESS.md`. Conventions :

- `/workspace/result/PROGRESS.md` — done / remaining index
- `/workspace/result/OUTPUT.md` — final synthesis / proposal / verdict
- `/workspace/result/notes/<topic>.md` — intermediate findings (optional)

## 4. When the research is done

Claude will propose the two-step back-copy in its last message. Run on your
host from the **main project root** (not from this bundle's folder) :

    # 1. Archive the session result back into the bundle source (snapshot) :
    cp -r ../<bundle_id>/result/. ./.devcontainer/research-bundles/<bundle_id>/result/

    # 2. Promote specific files to their final destination (manual, e.g.) :
    cp ../<bundle_id>/result/<file> ./<final-path>/

Then cleanup the sibling :

    rm -rf ../<bundle_id>/
    # Docker volumes for <bundle_id> can be pruned via `docker volume prune`.
    # DO NOT drop the main's `claude-creds` volume — it remains shared.
```

> 🚨 **When generating this file, replace every `<bundle_id>` with the
> actual computed value** (e.g. `dc-project-research-wtfcmd`).
> The literal string `<bundle_id>` MUST NOT appear in the generated
> `START.md` — same rule as for INSTRUCTIONS.md.

The prompt block (`<initial_prompt rendered…>`) is the template's
`initial_prompt` field with all `{{var}}` placeholders resolved against the
template's `prompts_for_user` answers and the standard variables
(`{{bundle_id}}`, `{{task_slug}}`).

Wrap the prompt block in a **4-backtick fence** (` ```` `) so any
triple-backtick code the user includes in the prompt stays renderable — same
trick as `/prepare-pr` (rule §"Why 4-backtick fence").

If the template has no `initial_prompt` field (older / hand-written template),
fall back to a generic prompt :

```
Read /workspace/INSTRUCTIONS.md and start the research. If
/workspace/result/PROGRESS.md exists, resume from it. Incremental synthesis
in /workspace/result/OUTPUT.md.
```

#### 3i. Write `bundle/.claude/settings.json`

Ship a Claude Code settings file that pre-allows the read-only operations and
web fetches a research session needs — without this, Claude in the research
container would prompt for permission on every `curl`, `grep`, `find`, etc.
The firewall remains the enforcement boundary (any blocked host returns 503
even if `WebFetch` is allowed in settings), so permissive read perms are safe.

```bash
mkdir -p "$BUNDLE/.claude"
cat > "$BUNDLE/.claude/settings.json" <<'JSON'
{
  "permissions": {
    "allow": [
      "WebFetch",
      "WebSearch",

      "Bash(curl:*)",
      "Bash(wget:*)",

      "Bash(ls:*)",
      "Bash(cat:*)",
      "Bash(head:*)",
      "Bash(tail:*)",
      "Bash(less:*)",
      "Bash(grep:*)",
      "Bash(rg:*)",
      "Bash(find:*)",
      "Bash(wc:*)",
      "Bash(file:*)",
      "Bash(tree:*)",

      "Bash(jq:*)",
      "Bash(yq:*)",
      "Bash(python3 -c:*)",
      "Bash(python3 -m json.tool:*)",
      "Bash(node -e:*)",
      "Bash(node --version)",

      "Bash(which:*)",
      "Bash(command -v:*)",
      "Bash(type:*)",
      "Bash(env)",
      "Bash(pwd)",
      "Bash(date:*)",
      "Bash(echo:*)",
      "Bash(printf:*)",

      "Bash(git log:*)",
      "Bash(git diff:*)",
      "Bash(git show:*)",
      "Bash(git status:*)",
      "Bash(git branch:*)",
      "Bash(git remote:*)",
      "Bash(git ls-files:*)"
    ]
  }
}
JSON
```

What is **NOT** pre-allowed (Claude will prompt) :
- Write commands : `rm`, `mv`, `cp` (so the user sees when files move).
- Package installs : `npm install`, `pip install`, etc. For
  `package-evaluation`, the user will approve once at the first install
  step ; subsequent installs are remembered for the session.
- `git push`, `git commit`, `git tag` — even though git push is firewalled
  for non-anthropics, we still want explicit confirmation.
- `gh pr create`, `gh issue create` — firewalled anyway.
- Editor / shell launches : `vim`, `nano`, `zsh` — interactive sessions
  should be explicit.

If the user wants to extend (e.g. allow `npm install` wholesale for a
package eval), they edit `bundle/.claude/settings.local.json` (gitignored
inside `.claude/`) or invoke `/fewer-permission-prompts` from within the
research container.

#### 3j. Write `bundle/secrets.env.template`

Every secret named, every value **MUST** be `REPLACE_ME` :

```
# Fill with sandbox/test keys. DO NOT commit.
# This file is the template — copy to .devcontainer/.env.local on the host
# and replace each REPLACE_ME with the real test value before opening the
# project in VS Code.

STRIPE_API_KEY_TEST=REPLACE_ME
STRIPE_WEBHOOK_SECRET=REPLACE_ME
```

For templates with no secrets (`doc-research`, `package-evaluation`), still
write the file with a comment explaining no secrets are required.

#### 3k. Copy workspace files (paths preserved)

For each pattern in the template's `files_to_copy` (or asked interactively if
no template), copy with `cp --parents` from `/workspace`. This preserves the
relative path so `/workspace/src/payment/Stripe.ts` in the research container
matches the main path exactly.

```bash
cd /workspace
for pattern in <patterns>; do
  # expand glob, skip missing
  for src in $pattern; do
    if [ -e "$src" ]; then
      cp --parents -r "$src" "$BUNDLE/"
    else
      echo "  (skipped : $src not found in main workspace)"
    fi
  done
done
```

For `doc-research` and `package-evaluation` with empty `files_to_copy`, skip
this step entirely.

#### 3l. Final sanity checks — no real secrets, no unresolved placeholders

```bash
# (a) secrets.env.template must contain only REPLACE_ME (or comments).
if grep -E '^[A-Z_]+=' "$BUNDLE/secrets.env.template" | \
   grep -vE '=REPLACE_ME$'; then
  echo "❌ secrets.env.template contains non-REPLACE_ME values — aborting"
  rm -rf "$BUNDLE"
  exit 1
fi

# (b) No unresolved template placeholders anywhere in the bundle.
# This catches the case where Claude forgot to substitute {{bundle_id}},
# {{task_slug}}, <bundle_id>, etc. in a generated file. policy.d/ and
# *.example files belong to the verbatim main copy and may legitimately
# contain such strings — exclude them.
if grep -rE '<bundle[_-]id>|<task[_-]slug>|\{\{[a-z_]+\}\}' "$BUNDLE" \
     --include='*.md' --include='*.json' --include='*.txt' --include='*.yaml' \
     --exclude-dir='policy.d' --exclude-dir='policy.local.d.example' \
     2>/dev/null; then
  echo "❌ unresolved template placeholders in bundle — aborting"
  rm -rf "$BUNDLE"
  exit 1
fi
```

### 4. Display next steps to the user

Print this exact block (substitute `<bundle-id>` everywhere — that's the
full concat `<main-dc-project>-research-<task-slug>` — and fill the counts) :

```
✅ Research bundle ready : .devcontainer/research-bundles/<bundle-id>/
   (bundle_id = <main-dc-project>-research-<task-slug>)

Contents :
  START.md               user entry-point : open VS Code, fill secrets,
                         copy-paste prompt to Claude (multi-session aware)
  INSTRUCTIONS.md        task brief read by Claude (goals, constraints,
                         success criteria, multi-session conventions)
  .claude/settings.json  pre-allow WebFetch + read-only Bash (curl, grep,
                         find, jq, git log/diff, …) — suppresses permission
                         prompts ; firewall stays the enforcement boundary
  .devcontainer/         verbatim copy of main + .env (DC_PROJECT=<bundle-id>,
                         CLAUDE_CREDS_VOLUME shared with main)
                         + firewall/domains.local.txt (<N> additions)
                         + firewall/policy.local.d/<M hosts>.yaml
  secrets.env.template   <K> secrets to fill (REPLACE_ME → real test values)
  result/                empty, will hold OUTPUT.md + PROGRESS.md
  <P workspace files>    paths preserved (e.g. src/<scope>/**, package.json)

Open `START.md` first — it contains the host setup steps and the prompt to
copy-paste into Claude once the research container is up.

Quick summary — run on the host, from your main project root :

  # 1. Copy bundle as a sibling folder
  cp -r .devcontainer/research-bundles/<bundle-id>/ ../<bundle-id>/

  # 2. Read ../<bundle-id>/START.md for the full kick-off procedure
  $EDITOR ../<bundle-id>/START.md

  # 3. Open in VS Code (claude-creds shared — Claude stays authenticated)
  code ../<bundle-id>/

  # 4. (After research, last session) Archive the result back into the
  #    bundle source — gitignored snapshot for future review :
  cp -r ../<bundle-id>/result/. ./.devcontainer/research-bundles/<bundle-id>/result/

  # 4b. Then promote specific deliverables to their final destination, e.g.
  #     cp ../<bundle-id>/result/<file> ./<final-path>/

  # 5. Cleanup the sibling :
  rm -rf ../<bundle-id>/
  # Docker volumes for <bundle-id> can be pruned via `docker volume prune`.
  # DO NOT drop the main's claude-creds volume — it stays shared.
```

## Constraints

- Bundle path is always `/workspace/.devcontainer/research-bundles/<bundle-id>/`
  (bind-mounted, gitignored). Do NOT write the bundle anywhere else. The
  bundle-id is the full concat `<main-dc-project>-research-<task-slug>`.
- **NEVER include real secrets in `secrets.env.template`.** Every value is
  `REPLACE_ME`. Step 3l enforces this — if you ever find yourself tempted to
  inline a real key, stop and ask the user.
- Committed `firewall/domains.txt` and `firewall/policy.d/*.yaml` are copied
  **verbatim** from the main. Never rewrite them. All research additions go
  through `firewall/domains.local.txt` + `firewall/policy.local.d/`.
- Domain additions MUST use the extended syntax (`[METHOD] host` or `host`
  only). Refuse `*.com` and other overly broad wildcards.
- Every `policy.local.d/<host>.yaml` MUST set `max_body_kb` explicitly
  (default 100, max ~1024).
- Workspace file copy MUST run from `cd /workspace` so `cp --parents` preserves
  relative paths.
- Never overwrite an existing bundle — refuse and ask for a different slug.
- Always print the host `cp -r` command in the final output. Without it, the
  user has no clear path from bundle to running research container.
- The bundle's `.devcontainer/.env` MUST set `CLAUDE_CREDS_VOLUME` to the
  resolved main volume name. Without it, `docker compose up` fails on the
  `external: true` volume.
- **Every generated file MUST contain the resolved `bundle_id` value, never
  the literal placeholder `<bundle_id>` / `{{bundle_id}}`.** When writing
  INSTRUCTIONS.md, START.md, `.env`, `domains.local.txt`, `policy.local.d/`,
  do a sanity grep at the end : `grep -RE '<bundle[_-]id>|\{\{bundle_id\}\}'
  "$BUNDLE"` MUST return zero matches. Same rule for `<task_slug>` /
  `{{task_slug}}` and any other template variable — substitute fully, never
  ship a placeholder.
- Chat with the user in French OK ; this skill body and all files Claude writes
  inside the bundle MUST be in English per CLAUDE.md.

## Failure modes

| Symptom | Cause | Mitigation |
|---|---|---|
| `bundle_id` directory already exists | Repeat invocation with same task-slug for the same main | Refuse, propose `task-slug-v2` (recompute `bundle_id`), confirm with user. Never overwrite. |
| `compile-policy.py --parse-only` fails | Bad `domains.local.txt` syntax | Abort step 3f, show the parser error, ask user to clarify. |
| `secrets.env.template` contains non-`REPLACE_ME` | Skill bug or LLM oversight | Step 3l wipes the bundle and aborts loudly. |
| `external: true` volume lookup fails when user does `docker compose up` | Main has never run (no claude-creds volume yet) | User-side issue : remind them to launch the main once before research. Skill can warn if `docker volume inspect claude-creds-...` fails. |
| `cp --parents` leaks paths outside the workspace | Pattern starts with `/` or `~` | Always `cd /workspace` first, refuse absolute patterns. |
| Bundle copy includes recursive `research-bundles/` | Missing exclude in rsync | The exclude list above is exhaustive ; do not edit it without bumping the skill version. |

## Templates

Three templates are committed under `/workspace/.devcontainer/skills/prepare-research/templates/` :

| Template | Use case |
|---|---|
| `new-api-integration.yaml` | Integrate a third-party API (Stripe, Linear, …) — POST allowed on target |
| `doc-research.yaml` | Pure read-only doc research (lib, framework eval) — GET only |
| `package-evaluation.yaml` | Evaluate an npm / composer / pip / cargo / go package — GET on registry + repo + docs |

Read the template file at `/workspace/.devcontainer/skills/prepare-research/templates/<name>.yaml`
to drive the prompts and bundle content. The template YAML schema :

- `template` : identifier (string)
- `description` : one-liner shown when listing templates
- `prompts_for_user` : list of `{var, label, default?}` to ask the user
- `domains_local_additions` : entries to write into `domains.local.txt`
- `policy_local_d_files` : list of `{host, body}` to write into
  `policy.local.d/<host>.yaml`
- `files_to_copy` : list of glob patterns relative to `/workspace/`
- `instructions_md` : body of `INSTRUCTIONS.md` (Handlebars-style placeholders)
- `secrets_template` : list of lines for `secrets.env.template`

Placeholders use `{{var_name}}` ; `{{#each list_var}}{{this}}{{/each}}` for
list expansion. The skill does the substitution before writing.
