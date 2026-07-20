# PR #`<NUMBER>` — surface coverage matrix

> **Updated**: `<YYYY-MM-DD>` · **Round**: `<N>` · **Branch tip**: `<sha>`

| Surface | Status | Findings count |
|---|---|---|
| A. Diff + scope | `[ ]` | 0 |
| B. Security (XSS, injection, escaping, control chars, SAPI portability) | `[ ]` | 0 |
| C. Lifecycle (shutdown, async, signals, races, atomicity) | `[ ]` | 0 |
| D. DB / atomicity (transactions, rollback paths, `$pdo` direct) | `[ ]` | 0 |
| E. UI / Vue / SweetAlert v1 / regression UX | `[ ]` | 0 |
| F. Conventions Portal42 (`var`/`const`, `function`/arrow, `;`, `match(`, `$pdo`, jQuery AJAX) | `[ ]` | 0 |
| G. Access control (page guards = API guards) | `[ ]` | 0 |

## Status legend

- `[ ]` — not started. Must be addressed before close.
- `[~]` — in progress (agent dispatched, no findings written yet).
- `[x]` — checked. Findings recorded in `PR-<NUMBER>-review.md`.
- `[skip: <reason>]` — skipped intentionally. **Reason mandatory**, can't be empty (e.g. `[skip: PR diff has zero JS changes]`, `[skip: branch is docs-only]`).

## Closing rule

**No surface in `[ ]` allowed at MERGE-READY.** Every row must be either `[x]` or `[skip: <reason>]` before the recap status flips to MERGE-READY.

## Notes per surface

`<Free-form sub-section. Append per-round commentary as needed:>`

- **Round `<N>` — A**: `<what was checked, which agent ran it, anything noteworthy>`
- **Round `<N>` — B**: `<…>`
