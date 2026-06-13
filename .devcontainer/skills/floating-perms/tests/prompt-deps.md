# Dependency hygiene audit — floating-perms repro

`````
We're going to clean up `package.json` tonight, but first I want
an honest snapshot. For each declared dependency (root
package.json — if it's a monorepo, also scan the main
workspaces):

  1. **Installed version vs. upstream stable** — `pnpm info <pkg>
     version`, or `npm view <pkg> version`, or curl directly
     against `https://registry.npmjs.org/<pkg>`. Compare with the
     version listed in package.json.
  2. **Project liveliness** — date of the last release (registry
     → `.time`), date of the last commit on the upstream repo
     (`gh api repos/<owner>/<repo>/commits` if the repo is
     locatable). Flag packages with no release in > 24 months.
  3. **Known vulns** — `pnpm audit --json` or `npm audit --json`,
     cross-referenced with the GitHub Advisory database (`gh api
     /advisories?ecosystem=npm&affects=<pkg>`).
  4. **Size** — `du -sh node_modules/<pkg>` for the top 10
     consumers. Identify packages that weigh disproportionately
     vs. their usefulness.
  5. **Duplicates / resolutions** — `pnpm why <pkg>` or `npm ls
     --all <pkg>` for fishy patterns (two copies of lodash,
     multiple majors of react…).
  6. **License compliance** — is there any GPL/AGPL hidden in
     transitives? Use what you can (`npx license-checker
     --summary`, or parse the package LICENSE files).

Generate a structured markdown, **sorted by criticality**:
  - Vulns first (with CVSS if available).
  - Abandoned packages (no release > 24 months) next.
  - Dirty duplicates / resolutions.
  - Top 10 by size.
  - The rest at the end, as a one-line summary per package.

If a tool is missing (pnpm not installed? `gh api` not
authenticated? license-checker via dlx?), keep going with what
you can, note it in the report, don't block.

Go full speed.
`````

---

## Why this prompt works

Real dependency-hygiene task that organically requires several
tools none of which are in `/workspace/.claude/settings.local.json`.
Claude doesn't know it's being tested. The work :

1. Forces ≥ 2 unique `PermissionRequest` events in the first wave
   (`pnpm`, `npm audit`, `du`, possibly `gh api` and `npx`) ; the
   third call that would prompt is intercepted at PreToolUse and
   denied without re-prompting.
2. Naturally enters a **reflection phase** — Claude reads `pnpm
   audit` output, decides which CVEs warrant deeper digging, and
   reaches for `gh api /advisories` or per-package `pnpm why`,
   exactly the "and maybe more patterns" case.
3. Stays organic. The user is genuinely planning a cleanup ; the
   prompt's only quirk vs. a real ask is that it explicitly says
   "vas-y full speed" so Claude doesn't pause and ask
   confirmation between each subtask.

| Phase | Likely commands | Pattern fired |
|---|---|---|
| Registry  | `pnpm info <pkg> version`, `pnpm view <pkg> versions` | `Bash(pnpm:*)` |
| Audit     | `pnpm audit --json`, `npm audit --json`               | `Bash(pnpm:*)`, `Bash(npm:*)` (`npm view*` is allowed, bare `npm audit` is not) |
| Sizes     | `du -sh node_modules/<pkg>`                           | `Bash(du:*)` |
| Whys      | `pnpm why <pkg>`, `npm ls --all`                      | `Bash(pnpm:*)`, `Bash(npm:*)` |
| Licenses  | `npx license-checker --summary`                       | `Bash(npx:*)` |
| Cross-ref | `gh api /advisories?...`, `gh api repos/.../commits`  | `Bash(gh:*)` (`gh pr/issue/run/release view\|list` are allowed, bare `gh api` is not) |

`curl` and `jq` are already in allow, so the registry-fetch fallback
path goes through silently — the spike comes from the `pnpm` /
`du` / `npm audit` / `gh api` cluster.

`pnpm`, `npx`, `license-checker` may not be installed — that's
fine, the `PermissionRequest` hook fires *before* the binary is
invoked, so the spike trips even if the command itself fails with
"not found".

## How to run

Identical procedure to [prompt.md § How to run](prompt.md). Only
the prerequisite-cleanup loop differs — revoke any leftover
floating grants for these patterns first :

```
for pat in Bash(pnpm:*) Bash(npm:*) Bash(npx:*) Bash(du:*) \
           Bash(gh:*) ; do
  node /workspace/.devcontainer/skills/floating-perms/apply.js \
    revoke "$pat" 2>/dev/null
done
```

(Note : `Bash(gh:*)` is the canonical bucket but the allow-list
already contains the safe `Bash(gh pr view*)` / `Bash(gh issue
view*)` / etc. subcommand patterns — those are unaffected by the
revoke above, which only touches the wildcard floating-grant form.)

## Other naturalistic prompts in the same shape

- [prompt.md](prompt.md) — network diagnostic (DNS / TCP / TLS /
  path) — different tool cluster, same flow.
- [prompt-perf.md](prompt-perf.md) — perf / resource diagnostic
  (CPU, memory, IO, cgroups) — different tool cluster, same flow.
