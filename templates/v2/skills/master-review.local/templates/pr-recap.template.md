# PR #`<NUMBER>` — recap

> **Status**: `<DRAFT | IN-REVIEW round N | MERGE-READY>`
> **Branch**: `<branch-name>` → `<base-ref>`
> **Tier**: `<T1 | T2 | T3 | T4 | T4+>`
> **PR URL**: `<gh url>`

> **For the next session.** Read this file first. The current round-`<N>` review file [PR-`<NUMBER>`-review.md](PR-<NUMBER>-review.md) carries the detailed findings of the round in flight. This recap is the canonical state document — overwrite it again at the next handoff.

## Scope

`<1-2 paragraphs: what this PR is, why it exists, the user-facing or system-level surfaces it touches. End with the PR URL again if useful.>`

## State at handoff

- **Branch tip**: `<sha>` "`<commit subject>`"
- **Origin sync**: `<in sync | N commits behind/ahead | force-push at end of round N>`
- **Last review round**: round `<N>`, `<YYYY-MM-DD>`
- **History events**: `<e.g. force-push round 6, filter-branch msg-filter — drop review vocabulary; or: none>`

## Decisions tables — DO NOT re-flag these

> Once a round is closed, its table is **frozen** — never re-edit prior rounds. New decisions go in a new round-`<N>` table below.

### Round `<N-1>` decisions

| # | Item | Decision | Fix commit |
|---|---|---|---|
| R`<N-1>`-1 | `<finding short title>` | `<Fixed | Declined | Documented | Kept on purpose>` | `<sha>` (`<short label of what the commit actually does>`) |
| R`<N-1>`-2 | `<…>` | `<…>` | `<sha or — if no commit>` |

### Round `<N>` decisions (current)

| # | Item | Decision | Fix commit |
|---|---|---|---|
| R`<N>`-1 | `<finding short title>` | `<status>` | `<sha>` (`<short label>`) |
| R`<N>`-2 | `<…>` | `<…>` | `<…>` |

## Open at start of round `<N+1>`

- R`<N>`-X — `<one-line summary>`. Why still open: `<reason — e.g. needs runtime validation, deferred to next sprint, needs design review>`.
- R`<N>`-Y — `<…>`.

If empty: `<None — round N reached plateau (T2/T3: <3 new findings ×2; T4+: <2 new findings ×3). MERGE-READY.>`

## Where to look fresh — round `<N+1>` angles

- **Surfaces still in `[ ]` or `[~]`**: `<list from PR-<NUMBER>-surfaces.md>`.
- **Adversarial framings to activate**: `<F1 fatal-path | F2 generalization sweep | F3 gap analysis | F4 round-vs-round diff | F5 timing/ordering>`.
- **Diff angles**: `<files / commit ranges the next round should target — e.g. "the deferred commitPurge introduced in 09a13c9b9 — verify FIFO ordering of register_shutdown_function chain">`.
- **Open questions for the user**: `<anything blocking that needs human input>`.
