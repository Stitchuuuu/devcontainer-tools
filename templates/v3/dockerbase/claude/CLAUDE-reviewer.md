# Claude Code — Reviewer Mode

## Project-Specific Rules

See `CLAUDE-dev.md` for generic dev guidelines (plan mode, simplicity, surgical changes, verification, commits) and `CLAUDE-project.md` for project-specific conventions (stack, repo layout, environment, wizard).
These rules apply in reviewer mode as well.

If `CLAUDE-project.md` has not been configured yet (first-run wizard not done), follow the wizard there first.

---

## Critical Rules

**ALWAYS:**
- Add `// @REVIEW:` annotations on every changed line (see Code Review below)
- **Remove ALL `// @REVIEW:` before committing** — never commit code containing `@REVIEW:`

**NEVER:**
- Commit secrets or credentials (`.env`, API keys, passwords)
- Commit `CLAUDE.md` or `.devcontainer/` — local dev files only

---

## Git & Branching

- **All text (commits, PRs, tags, branch names, code comments, documentation) must be in English.**
- Format: `feat/<desc>`, `fix/<desc>`, `refactor/<scope>`
- **Specific names only.** `feat/settings-accordion-layout` (good) — `feat/quentin-small-fixes` (bad).
- **Before starting work:** confirm the exact scope with the user, then propose a branch name before creating it.
- **One PR = one scope.** If the user requests an unrelated fix/feature mid-work, propose a separate branch.

### PR Format

```markdown
## Summary
- Brief description

## Test plan
- [ ] Testing steps

🤖 Generated with [Claude Code](https://claude.com/claude-code)
```

### Tag Description Format

Tags are displayed in a centered SweetAlert — **no dashes/bullets**.

- No `v` prefix — use `1.63.0`, not `v1.63.0`
- **Line 1:** short description (shown in select preview, must NOT contain the tag/version name)
- **Line 2:** empty
- **Line 3+:** one change per line, format `Scope: description`

```
UI Overhaul

Sidebar: 4 sections + order search button
Settings: accordion layout + search/filter
Gear menu: 5 categorized sections
```

Tag naming: `1.53.0` (prod), `1.53.0-b.1` (beta), `1.53.1` (hotfix).

### GitHub Authentication (gh-secure)

The devcontainer supports two auth modes, chosen at first build:

- **Standard**: `gh auth login` in the terminal, full access
- **Advanced**: read-only PAT + GitHub App for scoped PR write access

In Advanced mode:
- `gh pr create/edit/comment` are intercepted by a shell function → routed through a root-only wrapper → temporary installation token generated, used, and auto-revoked
- All other `gh` commands use the read-only PAT directly
- The write token never leaves root process memory

**Creating a PR in Advanced mode:** use `gh pr create` normally — the wrapper handles the token escalation transparently.

**If `gh pr create` is blocked** (read-only token, no gh-secure): ask the user to push the branch and create the PR manually.

**Reconfiguration:**
- Auth mode: delete `.devcontainer/.configured-auth` and rebuild
- Claude mode: delete `.devcontainer/.configured-claude-mode` and rebuild

---

## Security — Access Control Verification

**During review, ALWAYS verify** that every API endpoint and page has proper access control.
Flag it if: an endpoint has no protection at all, or the API and its calling page have mismatched guards.

---

## Code Review — Process

### Step 1 — Markdown Recap (BEFORE any code change)

Produce a `PR-<number>-review.md` summarizing:
- Corrections to make (blocking / non-blocking / pre-existing)
- What was verified OK
- Feature access (pages, roles, conditions, navigation)

**This recap must be validated by the user before any code intervention.**

### Step 2 — @REVIEW: Annotations

**MANDATORY:** When modifying code, add `// @REVIEW:` at the end of every changed line explaining the change. Do this simultaneously with the modification, not as a separate pass.

```javascript
const app = me.Vue({ // @REVIEW: var → const
	methods: {
		addGroup() { // @REVIEW: function() → method shorthand
			this.query({ // @REVIEW: V.query → this.query (arrow allows this)
				callback: (res) => { // @REVIEW: function(res) → arrow
					const msg = `Group "${name}" created` // @REVIEW: + → template literal
```

```php
if ($action === 'create') { // @REVIEW: == → ===
$db = Database::get(); // @REVIEW: global $pdo → Database::get()
exit(json_encode(['error' => 'fail'])); // @REVIEW: exit string → json_encode
```

### Step 3 — Strip Before Commit

**MANDATORY — never commit `@REVIEW:` annotations.** Strip them:

```bash
grep -rn "@REVIEW:" --include="*.js" --include="*.php" .   # check
sed -i 's/ \/\/ @REVIEW:.*//g' <file>                      # strip
```

If any remain after stripping, do not commit — fix first.

---

## DevContainer Phase 3 — review checklist

When reviewing a PR that touches `.devcontainer/` (or any file that interacts with the firewall, secrets, or lifecycle), step through this list before approving:

### Firewall / allowlist changes

- [ ] **No new POST host in committed `domains.txt`** — POST allowlist is intentionally limited to 4 targets (api.anthropic.com, *.statsig.com, sentry.io, github.com/anthropics/*.git/git-upload-pack). New POST hosts must use a research project, not the main allowlist. If the diff adds a POST elsewhere, ask "why not `/prepare-research`?" and block until justified.
- [ ] **Additions to `domains.d/<eco>.txt`** match what `extract-auto-dependencies` would produce — these files are generated by `/scan-deps`. If they were edited by hand, ask why and require regeneration.
- [ ] **No edits to `policy.compiled.yaml`** — it's a build artifact written by `compile-policy.py` at every boot. Any change here is futile (overwritten) or a sign the contributor doesn't understand the pipeline.
- [ ] **No new `firewall/policy.yaml`** — that file does not exist since A1.1; advanced rules live in `policy.d/<host>.yaml` (one file per host).
- [ ] **`domains.local.txt` modifications** are coherent with the philosophy: permanent overrides committable OR per-dev gitignored — never time-limited (no TTL semantics; if you don't want it forever, don't commit it).
- [ ] **Wildcard `[*]` host methods** appear only in `policy.local.d/<host>.yaml` overrides, with a justification comment. Never in `domains.txt` committed.

### Dependencies

- [ ] If `package.json` / `composer.json` / `pyproject.toml` / `requirements.txt` / `Cargo.toml` / `go.mod` was modified, **verify `/scan-deps` was run**:
  - `.devcontainer/firewall/domains.d/<eco>.txt` should reflect the new deps
  - `.devcontainer/scan-deps/<unix-ts>-<eco>.md` audit trail should exist (gitignored, so check the PR description references it)
  - `.devcontainer/scan-deps/.last-scan.json` timestamp should be > the manifest mtime in the PR
- [ ] Look for suspicious new deps: typosquats (e.g. `lodahs` vs `lodash`), unmaintained packages, packages with postinstall scripts. Flag in the review.

### Git / PR workflow

- [ ] **The PR draft does not contain `git push` or `gh pr create` instructions** — those are host-side. The PR was created via `/prepare-pr` + host `pr-from-draft` if the workflow is followed.
- [ ] No `--no-verify`, `--no-gpg-sign`, or hook-skipping flags in commit / push commands.
- [ ] Commit messages are self-contained (no `session XX`, `phase Y`, `plan Z` references) unless the user explicitly asked.

### Secrets

- [ ] No `.env`, `.env.local`, `*.pem`, `*.key`, `credentials.json` in the diff.
- [ ] No hardcoded tokens in source code (grep for `sk_test_`, `ghp_`, `eyJ`, etc.).
- [ ] No echo / print of env vars that could include tokens.

### Lifecycle / scripts

- [ ] New behaviour in lifecycle scripts (`initialize.sh`, `on-create.sh`, `post-create.sh`, `post-start.sh`, `shell-init.sh`) is **idempotent** — re-runnable without side effects. See [knowledge/INDEX.md § Idempotency contracts](../knowledge/INDEX.md#idempotency-contracts).
- [ ] No `sed -i` in host-side scripts (BSD vs GNU mismatch) — use `awk + temp + mv` instead.
- [ ] New mitmproxy addons import `ruamel.yaml`, not `import yaml` (PyInstaller bundle ships ruamel only).

### Testing

- [ ] `bash .devcontainer/tests/diagnose.sh` is all-green (the PR author confirmed in the description, or you ran it locally).
- [ ] New behaviour has at least one `pass_if` assertion in `diagnose.sh`.
