# PR #`<NUMBER>` — review round `<N>`

> **Date**: `<YYYY-MM-DD>`
> **Branch tip**: `<sha>` "`<commit subject>`"
> **Distance from main**: behind `<N>` · ahead `<N>` · leakage `<N>`
> **Tier**: `<T1 | T2 | T3 | T4 | T4+>`
> **Reviewer mode**: `<fresh-eye | resume --surfaces=<letters>>`

> Round `<N>` overwrites round `<N-1>` in this file. Closed-round decisions are archived in [PR-`<NUMBER>`-recap.md](PR-<NUMBER>-recap.md). If you're starting a new round in a fresh session, read the recap first, then this file.

## Findings

### R`<N>`-1 — `<short title>`

- **Severity**: `<BLOCKING | HIGH | MED | LOW>`
- **Surface**: `<letter — name, e.g. C — Lifecycle>`
- **File**: [`<path>`](`<path>`#L`<line>`)
- **Description**:

  `<what's wrong, why it matters, who would feel the impact (oncall? attacker? heir dev in 6 months?). 1-3 sentences.>`

- **Fix proposed**:

  `<concrete change. Reference exact functions/lines if useful. Note alternatives only if genuinely competing.>`

### R`<N>`-2 — `<short title>`

- **Severity**: `<…>`
- **Surface**: `<…>`
- **File**: [`<path>`](`<path>`#L`<line>`)
- **Description**: `<…>`
- **Fix proposed**: `<…>`

`<Repeat per finding. Numbering is round-scoped — R<N>-1, R<N>-2, …>`

## Surfaces couvertes

- `[ ]` A — Diff + scope
- `[ ]` B — Security
- `[ ]` C — Lifecycle
- `[ ]` D — DB / atomicity
- `[ ]` E — UI / Vue / SweetAlert v1
- `[ ]` F — Conventions Portal42
- `[ ]` G — Access control

Tick `[x]` after a surface is checked. `[~]` if started but not closed. `[skip: <reason>]` for intentional skips (reason mandatory). Mirror this state into `PR-<NUMBER>-surfaces.md`.

## Recommandation

- **Merge-ready**: `<Y | N>`
- **If N — what's needed**: `<list of open findings + surfaces still in [ ] or [~]>`
- **Plateau status**: `<new findings this round> / <threshold for tier>` — `<reached | not yet>`
  - T2/T3 threshold: 2 consecutive sessions with `< 3` new non-cosmetic findings.
  - T4+ threshold: 3 consecutive sessions with `< 2` new findings.
