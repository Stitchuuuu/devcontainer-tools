# Master Review — Config Portal42 POS

<!--
  This file is the per-project configuration consumed by the GENERIC
  /master-review command. It declares everything Portal42-specific:
  custom agents (#6 #7 #8), the surface matrix (A-G), tier-weight
  overrides, tactical framings (F1-F5), output paths, commit-hygiene
  regex, special-case files, and threshold overrides.

  Other projects can copy this file as a starter, replace the Portal42
  domain knowledge with their own, and ship.

  Resolution order (the command searches these paths in order):
    1. .devcontainer/claude/review-config.md   (this path)
    2. .claude/review-config.md
    3. review-config.md (repo root)

  Format spec lives inside `.claude/commands/master-review.md` under
  "Configuration File Specification". Section headings are
  case-sensitive H2 (`## `). The parser uses bash + awk only — no jq.
-->

## Project Meta

- **Name**: Portal42 POS
- **Language**: PHP 8.2 (no typed properties, no return types, no `match`)
- **Frameworks**: Vue.js 2 via `me.Vue()`, SweetAlert v1 (callback API), MySQL/MariaDB + Phinx, PHP-FPM + nginx
- **Conventions doc**: .devcontainer/claude/CLAUDE-reviewer.md
- **Dev doc**: .devcontainer/claude/CLAUDE-dev.md

## GitHub Review Threads

<!--
  Portal42 tracks review feedback in local PR-<n>-review.md files, not in
  GitHub native review threads. Disabling here drops Agent #4 from the
  generic launch list (S4 golden test showed 0/0/0 yield, 31 Bash calls).
  Other projects using GitHub-native review threads should leave this
  enabled (or omit the section — default is `true`).
-->

- enabled: false

## Tier Scoring Overrides

<!--
  Additive on top of the built-in lines/files-only base score. Use
  BLOCKING — abort review for files that should never be auto-reviewed
  (the command halts and asks for human review when the diff matches).

  The Signal column is treated as an extended regex matched against the
  PR's file list (`gh pr view --json files`). Use ERE syntax: pipe-
  alternation, anchored paths, etc.
-->

| Signal                                   | Weight                       |
|------------------------------------------|------------------------------|
| ^inc/                                    | +2                           |
| ^api/                                    | +2                           |
| (shutdown\|register_shutdown\|async\|signal\|fpm-master\|cron) | +3 |
| ^api/strain_webhook\.php$                | BLOCKING — abort review      |
| ^sql-migrations/                         | +3                           |
| ^scripts/pages/[^/]+\.js$                | -1                           |
| \.(md\|txt\|json)$                       | -1                           |
| ^assets/json/settings/                   | -1                           |

## Surfaces

<!--
  The audit matrix. Each surface ID (A-G) is one row. Trigger patterns
  are free-text grep recipes the agent uses to locate relevant code in
  the diff. Agent assigned points to a vanilla #1-5 or to a custom #6+.

  T3+ PRs MUST mark each surface as [checked] / [skipped: reason] in
  the recap before the round closes. T1/T2 PRs do not require the
  surface checklist.
-->

| ID | Surface           | Trigger patterns                                                                                  | Agent assigned |
|----|-------------------|---------------------------------------------------------------------------------------------------|----------------|
| A  | Security          | grep escapeHtml, htmlspecialchars, $_GET, $_POST, sendJSON, Content-Length, json_encode, safeHref | #6             |
| B  | Lifecycle         | grep register_shutdown_function, pcntl_, flock, ignore_user_abort, AsyncTask, ob_start            | #7             |
| C  | DB / Atomicity    | grep Database::get, begin, rollBack, smartInsert, smartUpdate, tempnam, uniqid, rename            | #7             |
| D  | UI / Vue          | grep me.Vue, this.query, swal, await swal, .then, .value                                          | #1             |
| E  | Conventions       | grep `var `, `function(`, `;$`, `match(`, `$pdo`, in_array, == / !=                               | #1             |
| F  | Access control    | grep IS_P42, IS_ADMIN, protectPage, requireRole, IS_DEV                                           | #6             |
| G  | SAPI portability  | grep `\bSTDERR\b`, `\bSTDIN\b`, `\bSTDOUT\b`, `fwrite\(STDERR`, `fgets\(STDIN`, `readline\(`      | #6             |

## Custom Agents

Custom agents are defined in [`agents/`](agents/) — one `.md` file per agent (frontmatter + body). To add a new agent, see [`agents/README.md`](agents/README.md) for the format and create `agents/agent-NN-<slug>.md`.

Agents currently defined :

- [agents/agent-06-security.md](agents/agent-06-security.md) — Security Portal42 (tier ≥ T3, surfaces A/F/G)
- [agents/agent-07-lifecycle.md](agents/agent-07-lifecycle.md) — Lifecycle & Atomicity (tier ≥ T3, surfaces B/C)
- [agents/agent-08-adversarial.md](agents/agent-08-adversarial.md) — Adversarial composite (tier ≥ T4+, all surfaces)

## Tactical Framings

<!--
  Sub-prompts injected into adversarial agents. Empirically validated
  against PR-1297 sessions (cf. plan-v2-deltas.md section 2). Reference
  by ID inside any custom agent's prompt.
-->

| ID | Framing verbatim                                                                                                                  | Target surfaces | Used by agents |
|----|-----------------------------------------------------------------------------------------------------------------------------------|------------------|------------------------|
| F1 | "What if this throws? OOM mid-fwrite? Parse error? E_ERROR? try/finally does NOT run on fatal — only register_shutdown_function does. Two-tier pattern required?" | B, C             | #7, #8                 |
| F2 | "This finding is specific to function X. Is it a PATTERN? Acquired-state-without-finally applies to locks, ob_start, transactions, ini_set, handler swaps, tempnam." | B, C             | #7, #8                 |
| F3 | "The codebase already has helper X. What sub-class is NOT covered? Attribute context vs text context? URL scheme? UTF-8 bytes? Say WHAT'S protected and WHAT ISN'T." | A, F             | #6, #8                 |
| F4 | "Don't summarize. Point to the exact line, exact change. What did this PR ADD or REMOVE? Show the line."                          | cross-cutting      | #6, #7, #8 (all)      |
| F5 | "When does this run? Init-time vs runtime? Which process — FPM worker, FPM master, cron, AsyncTask child, CLI? In what ORDER?"    | B, G             | #6, #7, #8             |

## Output Paths

<!--
  Where to write the round file, recap file, surfaces matrix. ${PR} is
  interpolated at command time.
-->

- Round file: PR-${PR}-review.md
- Recap file: PR-${PR}-recap.md
- Surfaces matrix: PR-${PR}-surfaces.md
- gh pr comment: enabled

## Commit Hygiene Regex

<!--
  POSIX ERE patterns the commit linter forbids in commit subject and
  body. Override the built-in defaults entirely if this section is
  declared. cf. plan.md section F-bis for the rationale (a commit
  message must describe the change, not the review numbering — the
  recap.md is the source of truth for "R4-1 -> commit X").
-->

- `(?i)round\s*\d+`
- `(?i)R\d+-\d+`
- `(?i)review\s+(fix(es)?|polish|done|round)`
- `(?i)PR-?\d+-(review|recap)`

## Special-case Files

<!--
  Files that block or warn before review. block review = command halts
  unless --force is passed. warn before review = command warns and
  asks for confirmation.
-->

- `api/strain_webhook.php` (block review): payment integration, must be human-reviewed (ref CLAUDE-reviewer.md "NEVER edit api/strain_webhook.php")
- `sql-migrations/.*\.php` (warn before review): existing migrations are immutable in prod; new migrations create new files only

## Override Threshold

<!--
  Confidence threshold (0-100) below which a finding is filtered out.
  Default 80 inherited from upstream /review. Lowered to 75 for the
  Portal42-specific custom agents because they are calibrated to err
  on the side of false positives — the lifecycle and security surfaces
  produce noisier candidates but the noise has historically been
  worth the signal.
-->

- Default 80
- 75 for agents 6-8
