# CLAUDE.md — Dev guidelines

For *implementation tasks* — writing code, fixing bugs, refactoring, building
features. Goal : reduce drift, over-engineering and false-done. **Bias
declared : caution over speed.** For questions, exploration, or "explain X" /
"where is Y", answer directly — no plan mode, no ceremony, no verification
loop ; switch to these rules the moment a "simple question" needs an edit.
Project rules (stack, conventions, environment) :
[CLAUDE-project.md](.devcontainer/claude/CLAUDE-project.md).

## 1. Plan Mode default

Enter plan mode for any non-trivial dev task — **3+ steps**, an
**architectural decision**, or where the right approach isn't obvious. It
covers *both* building and verification. Write the plan upfront : detailed
specs reduce ambiguity and let the user catch drift before code is written.
If something goes sideways mid-execution, **STOP and re-plan immediately** —
don't push through a plan that no longer fits reality.

**Multi-session work** (≥3 sessions — a feature rollout, a refactor across
many files, a migration) : propose the `/prepare-plan` skill. It scaffolds a
rollout directory (ROLLOUT + STATUS + LOG + EXISTING + sessions/) so progress
survives session boundaries.

**Session prompts and recaps live in the rollout directory**, under
`plans/<rollout>/sessions/` (gitignored), never inside a source subtree. And a
committed `.md` may only reference things that cannot disappear — no link or
path into `plans/`, a session file, or any other artefact outside the tree it
ships with. A committed document whose links rot is worse than one that never
had them.

## 2. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.** State your
assumptions explicitly and ask when uncertain ; if multiple interpretations
exist, present them rather than picking silently ; if a simpler approach
exists, say so and push back when warranted ; if something is unclear, stop,
name what's confusing, and ask.

This is the cheapest moment to catch a misunderstanding. If it slips through
anyway : **two failed corrections on the same point → stop patching.** Propose
/clear (or /rewind) plus a better prompt incorporating what was learned,
instead of a third in-place fix — accumulated corrections keep the failed
attempts in context and degrade everything after them.

**On a bug, suspect your own recently-touched code FIRST.** Never edit a
third-party or shared library on a hypothesis before you have audited your own
diff and cleared it. The shared library has many users and few recent changes ;
your diff has one user and many. Read the thing you just touched before you
touch the thing you didn't.

## 3. Simplicity First

**Minimum code that solves the problem. Nothing speculative.** No features
beyond what was asked, no abstractions for single-use code, no "flexibility"
that wasn't requested, no error handling for impossible scenarios. If you
write 200 lines and it could be 50, rewrite it. Self-check : *"Would a senior
engineer say this is over-engineered?"* **No laziness either** — find root
causes, not workarounds. The simplest solution is rarely the laziest one.

## 4. Surgical Changes

**Touch only what you must. Clean up only your own mess.** Don't "improve"
adjacent code, comments or formatting ; don't refactor what isn't broken ;
match existing style even if you'd do it differently ; if you notice unrelated
dead code, mention it — don't delete it. When your changes create orphans,
remove the imports / variables / functions *your* changes made unused, not
pre-existing ones. The test : **every changed line traces directly to the
user's request.** Anything else is scope creep — propose it separately.

## 5. Goal-Driven Execution

**Define success criteria. Loop until verified.** Turn vague tasks into
verifiable goals : "add validation" → "write tests for invalid inputs, then
make them pass" ; "fix the bug" → "write a test that reproduces it, then make
it pass" ; "refactor X" → "tests pass before and after". For multi-step tasks
state a brief plan with a verification per step (`1. [step] → verify:
[check]`). Strong criteria let you loop independently ; weak ones ("make it
work") require constant clarification.

Two assertion shapes that pass while the defect ships : **a test that asserts
only the *kind* of a thing cannot see a lost payload** — asserting that an
intent parsed as `search` says nothing about whether its argument survived, so
assert the argument too ; and **a test double carrying a method the real object
does not have doesn't miss the bug, it guarantees it** — the double must be no
richer than what it stands in for, keep one test on the real type.

## 6. Verification Before Done

**Never mark a task complete without proving it works.** Run the tests, check
the logs, demonstrate correctness with a concrete artefact (test pass, log
line, screenshot, output). When relevant, diff behaviour between `main` and
your changes. Ask : **"Would a staff engineer approve this?"** "It compiles"
isn't verification.

**Autonomous bug fixing.** Bug reports come with what you need — the failing
test, the error log, the stack trace. Point at the evidence, form a
hypothesis, resolve it, verify. Don't ask the user to walk you through the
diagnosis.

**Absence of evidence is not evidence.** Three ways a measurement lies by
omission : a log that does not instrument the suspected path can neither
confirm nor deny a defect on it (no lines is not a reading — prove the log
covers the path first) ; a throw inside an event handler can vanish with no
trace at all (no log line is not the same as no execution) ; and when a
verifier you wrote contradicts the user's direct observation, distrust the
tool, not the user, and go get the ground truth early — their screen is the
product.

**Trace from the reference, don't use the user as a console.** When the
implementation you are matching has readable source — a reference
implementation, an upstream bundle, a spec with code — a behavioural
discrepancy is traced *from that code*. One more probe for the user to run is
slower and spends their attention on something you could have read.

**Identify precisely before acting.** Tell two instances of the same binary
apart by **listening port**, never by process name — a name-based match catches
the wrapper, the shell, and itself. And before a destructive path command
(`rm -rf <dir>`), move out of the blast radius every artefact whose name shares
a prefix with `<dir>` — rename it, don't resolve to "be careful". A glob does
not know what you meant.

## 7. Subagent Strategy

Use subagents liberally to keep the main context clean : offload research,
exploration and parallel analysis ; for complex problems throw more compute at
it via parallel subagents, one tack each. The main thread keeps synthesis and
decisions. When in doubt, spawn a subagent rather than read twenty files in
the main context. **Verify a subagent's factual claims against the filesystem
before acting on them.**

## 8. Self-Improvement Loop

After any correction from the user, capture the rule so the mistake doesn't
repeat. **Trigger, not judgement call** : the user re-typing an instruction
already given once — same session or not, even reworded — IS the signal.
Capture it before the session ends. (Official threshold : "you type the same
correction you typed last session" → it belongs in the rules.)

Three layers by scope — **review all three at session start** :

- **[.devcontainer/LESSONS.md](../LESSONS.md)** (root symlink, **committed**)
  — project-wide patterns : recurring pitfalls, team conventions surfaced via
  correction, gotchas about the code.
- **`.devcontainer/LESSONS.local.md`** (**gitignored**) — personal or
  not-yet-generalisable. Safe default when ambiguous ; promote later.
- **auto-memory `MEMORY.md`** — cross-project preferences and feedback, not
  tied to this codebase.

Entry shape : one bullet per lesson — **rule** first, then *Why* and *How to
apply*.

## 9. Demand Elegance

For non-trivial changes, pause : **"Is there a more elegant way?"** If a fix
feels hacky, retry — *"Knowing everything I know now, implement the elegant
solution."* Skip it for obvious fixes. Challenge your own diff before
presenting it.

## 10. Commits

**Run tests before proposing the commit** — manual, automatic, or long suites
in the background while you proceed. A failing test is a not-done state : fix
it, don't commit and "address in follow-up". This is §6 at the commit step.

**Never `git commit` without an explicit user request.** When tests pass and
the change looks done, *propose* the commit — message included — and wait for
the user to confirm or amend it.

**Commit messages self-contained.** Describe what was added / modified / fixed
and why, in the change's own words. No rollout plans, session ids, phase
numbers or tracker artefacts — a commit is read by people without the plan
open (reviewers, future-you, `git blame`). Exception : the user asks for one.

**To commit one scope from a file that already carries unrelated WIP, stage
only those lines** — `git diff` the hunks you want and `git apply --cached`
them. Never the detour of backing the file up, `git checkout HEAD`, re-editing,
committing and restoring the backup : it destroys the working tree for the
duration and loses anything not in the backup.

## 11. Devcontainer signals

Some skills ship a `hooks.json` that `sync-skills` merges into Claude
settings at container boot ; their SessionStart / UserPromptSubmit hooks
inject `<system-reminder>` context surfacing state you can't detect
mid-conversation. **Treat these signals as authoritative for the state they
describe.** Each one proposes — never act autonomously, and if the user
declines or postpones, drop it for the session.

- **rollout-debt** (SessionStart, shipped by `prepare-plan`) — a plan has open
  🚧/📋 rows and nothing in its directory touched for more than
  `ROLLOUT_DEBT_DAYS` (default 7) : at a natural pause, propose closing it out,
  deferring or cancelling, and recording the decision in its STATUS.md. Never
  edit a STATUS.md autonomously, never start the work.
- **session-gap** (UserPromptSubmit, shipped by `session-gap`) — more than
  `SESSION_GAP_HOURS` (default 1) since the last event in this transcript : the
  prompt cache is cold and a rewrite costs ~2×. Answer the user's prompt first ;
  then, only if the remaining work is self-contained, propose moving it to a
  fresh session and offer to write the handoff prompt. Never end or clear the
  session yourself.

## 12. Project context bridge

These guidelines are *how* to do dev work. Stack, conventions and environment
live in [CLAUDE-project.md](.devcontainer/claude/CLAUDE-project.md) — read it
before starting. The visual fidelity loop is the `/visual-loop` skill.

## 13. Code style — perf + clarity

Meta-rule : **perf ≥ modern idioms, as long as readability holds.** §4 wins on
existing code. For code **you write** :

- **Single-read property access.** Any property read more than once in a scope
  is hoisted to a `const` at the top. Applies to deep chains (`a.b.c`) and to
  repeated `.length` alike.
- **No nested `if` on the same value.** Two levels of `if` testing the same
  variable against different thresholds collapse to a flat `if / else if`
  chain ordered from most restrictive to broadest.
- **Object iteration : `for…in` by default.** `Object.entries` / `keys` /
  `values` allocate an intermediate array on every call ;
  `for (const k in obj) { const v = obj[k] }` allocates nothing.
- **No `Map` / `Set` by default.** For lookup, deduplication and counting a
  plain object and an array are enough and faster. Before reaching for `Map`
  because the key happens to be an object, ask : is there a natural string
  identifier (id, name, uuid, path) ? `Map`/`Set` is justified only when
  semantics require it — keys that can't reduce to a string, `.size` without
  recompute, stable insertion-order iteration with frequent deletions.
- **Dependency-add hygiene.** Three conditions before `npm install` (or
  composer / pip / …) : **no known CVE** (never relaxed) ; **latest stable**,
  queried from the registry right before editing the manifest (`npm view <pkg>
  version`), never from memory ; **a trust signal** — actively maintained,
  widely adopted, mature and stable, or small enough to audit in minutes (*not
  maintained ≠ outdated*). No per-platform native binaries unless the project
  already repairs them for both sides of the bind mount.

## 14. Notification body convention

When you finish a turn — about to **Stop**, no tool call queued, no question
for the user — append a single recap line at the very end of your reply,
formatted exactly as :

```
**Recap** — <summary ≤ 80 chars>
```

It is parsed by notify-queue's hook (`/opt/devcontainer/base/skills/notify-queue/hook.js`,
baked) as the desktop-notification body ; without it the hook falls back to an excerpt of
your first usable line. **≤ 80 characters, action-oriented, past tense, raw
UTF-8** — type `é` and `—` directly, never as `\uXXXX` escapes or HTML
entities. No bold, emoji or backticks :
it renders as system text. Skip it when the reply is a bare acknowledgement,
and never add it mid-turn — tool calls, plan proposals and clarifying
questions aren't Stop events.
