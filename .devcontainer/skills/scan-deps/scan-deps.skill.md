---
description: Review project dependencies. Runs the deterministic extract-auto-dependencies (npm + composer) then layers AI analysis on top — flag suspicious deps, postinstall scrutiny, POST candidates, suggest /prepare-research for scope outside the baseline.
argument-hint: "[--no-extract] [--offline] [--ecosystem npm|composer|python|cargo|go]"
---

# /scan-deps — two-layer dependency audit (extract-auto + AI review)

The main devcontainer runs a Level 1 strict firewall with a Claude-only
baseline (17 hosts). Project dependencies live in a separate, additive
layer : `firewall/domains.d/<eco>.txt`, one file per ecosystem.

This skill is **two-layer** :

1. **Layer 1 — deterministic extraction** : invoke
   `bash .devcontainer/skills/scan-deps/extract-auto-dependencies --ecosystem all`
   (F2 implements npm + composer ; python/cargo/go fall through to AI handling).
   Rewrites `domains.d/<eco>.txt` from `package.json` / `package-lock.json` /
   `node_modules` / `composer.lock` / `pyproject.toml` / etc.
2. **Layer 2 — AI analysis** : audit what the deterministic extractor
   produced, flag suspicious deps, POST candidates, and other edge cases
   the script can't reason about.

## When to use

- Banner reminds : "Project manifests changed since last firewall extract".
- User adds/removes a dep and wants a confidence check before
  `init-firewall.sh` reload.
- User suspects a new dep needs POST elsewhere → spawn research bundle.
- User says : "scan les deps", "vérifie l'allowlist firewall", "audit dep
  changes since last extract".
- Pre-flight check before merging a branch that touches `package.json` /
  `composer.json` / `pyproject.toml` / `Cargo.toml` / `go.mod`.

## Process

### Step 1 — invoke extract-auto-dependencies (Layer 1)

Pass `--ecosystem` if the user specified one (default : all detected
ecosystems). F2 implementation : npm + composer have full extractors.
For python / cargo / go, `extract-auto-dependencies` exits silently
and the skill takes over (Step 2 + 3 do everything).

```bash
bash /workspace/.devcontainer/skills/scan-deps/extract-auto-dependencies \
     --ecosystem all
```

Pass `--offline` if the composer extractor's online HEAD requests to
api.github.com would fail in your environment (no network, or firewall
blocks the first-run bootstrap before composer.txt is loaded). composer
then falls back to source-clone via github.com `/V/R.git/*` and loses
coverage only for renamed/transferred repos whose dist URL 302s to
`/repositories/<id>/`.

Capture stdout (summary) + read `firewall/domains.d/<eco>.txt` files
to inspect what landed in the allowlist.

If the user passed `--no-extract`, skip this step (audit existing state).

### Step 2 — for ecosystems WITHOUT a deterministic extractor

For python / cargo / go, manually invoke the corresponding parser
(the F1 logic, now consolidated into the skill body) :

- Detect manifest : `pyproject.toml`, `requirements.txt`, `Cargo.toml`,
  `go.mod` under `/workspace` (depth-bumped : 3, 5, 8, 10 ;
  excludes `node_modules`, `vendor`, `.git`, `research-bundles`).
- Enumerate deps from manifest + lock file. Emit per-dep paths on the
  matching registry (pypi → `/simple/<pkg>/*` ;
  crates → `/api/v1/crates/<name>*` ; go.mod → `<host>/<owner>/*`).
- Walk `vendor/` / `site-packages/` / `target/` / etc. if present, for
  `.repository.url` equivalent and postinstall hints.
- Aggregate into a draft `domains.d/<eco>.txt` block. Show the user the
  proposed content and ask for confirmation before writing. The skill
  **never auto-writes** these files for non-npm ecosystems — the user
  reviews the content first.

### Step 3 — AI analysis (Layer 2 — value-add on top of Layer 1/2)

For each ecosystem :

1. **Suspicious deps** — flag :
   - scope-guesses where the heuristic can be wrong (e.g., `@types/foo`
     maps to `microsoft/DefinitelyTyped`, not `github.com/types/foo`)
   - dep names close to typosquatted variants (lodash vs lodahs, react
     vs reacht, …)
   - dep with no `repository.url` or with one pointing to a defunct host

2. **Postinstall scrutiny** :
   - `.scripts.postinstall` containing IP literals instead of domains
   - `.binary.host` pointing to non-mainstream CDNs (warn the user)
   - postinstall using `curl | sh` patterns

3. **POST candidates** :
   - deps that obviously need POST (Stripe SDK, Sentry SDK, Datadog,
     PostHog, Segment, mixpanel, …) → suggest `/prepare-research
     <slug>` rather than baseline extension
   - REST clients consumed in source code that target external APIs
     (best-effort — `grep -r 'fetch\|axios' src/` to spot patterns)

4. **Coverage gap** :
   - Compare `policy.compiled.yaml` (after the firewall reload) vs the
     deps detected. Surface gaps (e.g., a dep declared but no path in
     compiled).

5. **Docs curation review** :
   - The mapping covers ~30 popular libs. If a notable lib is detected
     but missing from `ecosystem-docs.txt`, suggest adding it to
     `extractors/lib/npm-docs-mapping.txt`.

### Step 4 — write structured audit

Generate `.devcontainer/scan-deps/<unix-ts>.md` (atomic `.tmp` + `mv`) :

```markdown
# Scan-deps audit — <ISO ts> (unix: <ts>)

## Layer 1 — deterministic extract
- Ecosystem(s) processed : <list>
- domains.d/npm.txt : <N> deps, <M> docs
- domains.d/ecosystem-docs.txt : <K> entries

## ✅ Auto-extracted (no action required)
<recap by category>

## 🟡 AI flagged (review needed)
- <scope-guess host>: heuristic, validate before commit
- <suspicious dep>: <reason>

## ❌ POST candidates (spawn research)
- <dep>: needs POST on <api host> → suggested
    /prepare-research <slug>

## 💡 Notes
<docs mapping gaps, coverage gaps, etc.>

## Next steps
- Review `git diff firewall/domains.d/`
- Reload firewall : `sudo /usr/local/bin/init-firewall.sh`
  (or rebuild container)
- For POST candidates : run `/prepare-research <slug>` in a fresh chat
```

Also update sentinel `.devcontainer/scan-deps/.last-scan.json` (kept for
audit history — not used by the F2 banner, which is mtime-based).

### Step 5 — present to the user

Surface the audit in chat. If the audit shows no flagged items :

> ✅ Audit clean. Run `sudo /usr/local/bin/init-firewall.sh` to reload
> the firewall, or rebuild the container. Audit : `.devcontainer/scan-deps/<ts>.md`.

If items are flagged, walk the user through them and ask which option
to take :
- **A** — accept as-is, reload firewall (auto-extract already did the work)
- **B** — spawn research bundle for POST candidates
- **C** — manually edit `firewall/domains.d/<eco>.txt` (rare ; usually
  `extract-auto-dependencies` is the source of truth)

## Constraints

- **Layer 1 must be invoked first** unless `--no-extract` is passed.
  This guarantees the allowlist on disk reflects current manifests.
- **No automatic edits** to `firewall/domains.txt` (the Claude-only
  baseline). Only `domains.d/<eco>.txt` is touched (by
  `extract-auto-dependencies` for npm + composer, by the skill for
  other ecosystems with user confirmation).
- **Deterministic ≠ offline.** The Layer 1 contract is « no AI / no
  human judgement », not « no network call ». When a deterministic
  fact requires a network call (e.g. composer's numeric-ID redirect
  resolution), the extractor MAY make it provided it offers an
  `--offline` opt-out. npm is 100 % offline ; composer is
  online-by-default with `--offline` available.
- **Audit only writes** under `.devcontainer/scan-deps/` (audit `.md`
  + sentinel JSON).
- **`.local` layer untouched** — `domains.local.txt` /
  `policy.local.d/` remain for user-specific overrides outside the
  project baseline.

## Failure modes

- `extract-auto-dependencies` not present or fails → fall back to
  pure-AI parsing for npm too (less precise, surfaced as a warning).
- `policy.compiled.yaml` absent → coverage gap analysis skipped, audit
  notes the gap.
- 0 manifests detected → print "no manifests under /workspace, nothing
  to scan" and exit without writing.
- composer extractor running with the firewall already locked down for
  the FIRST time : the online HEAD on `/repos/V/R/zipball/<sha>` gets
  403 `endpoint_not_matched` (composer.txt isn't compiled yet) and
  curl returns an empty redirect_url. Numeric-ID resolution skips
  silently — composer falls back to source-clone via github.com
  `/V/R.git/*` for renamed/transferred repos. Re-running scan-deps
  after the firewall reload picks up the redirects on the second pass.
  Alternatively pass `--offline` from the start.

## Example invocation

```
user: /scan-deps
assistant: [runs extract-auto, reads domains.d/, runs AI analysis]
          Layer 1 (deterministic) :
            npm.txt : 15 deps + 9 docs.
          Layer 2 (AI review) :
            🟡 @types/node maps to microsoft/DefinitelyTyped (not github.com/types/*)
            ✅ no POST candidates
            ✅ no suspicious postinstall
          Audit : .devcontainer/scan-deps/1747214580.md

          Want me to update the docs mapping for @types/* → microsoft/DefinitelyTyped?
```

## Files in scope

- Reads : `firewall/domains.txt` + `firewall/domains.d/*.txt` +
  `firewall/policy.compiled.yaml` ; project manifests + lock + node_modules +
  composer.lock.
- Writes : `firewall/domains.d/npm.txt` + `firewall/domains.d/composer.txt`
  (via extract-auto), maybe `firewall/domains.d/<other-eco>.txt` (skill
  with confirmation), `.devcontainer/scan-deps/<ts>.md` (audit),
  `.last-scan.json` (sentinel).
- Never writes : `firewall/domains.txt`, `firewall/policy.d/`,
  `firewall/domains.local.txt`, `firewall/policy.local.d/`.
