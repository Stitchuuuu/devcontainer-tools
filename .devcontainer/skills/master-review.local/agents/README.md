# Custom Agents — File-per-agent format

Each `agent-NN-<slug>.md` file in this directory defines one custom Sonnet
sub-agent dispatched at Step 3 of the master-review flow. Custom agents start
at #6.

Agents 1-5 are the generic `/review` agents (CLAUDE compliance, shallow bug
scan, blame, prior PRs, comments) and live inline in `master-review.skill.md`
Step 3 → "Generic agents". Their prompt text is mirrored here for
discoverability under [`upstream/`](upstream/) — see that folder's README for
why those files are not dispatched by the walker.

## File format

YAML frontmatter, blank line, then the prompt body verbatim:

```markdown
---
id: 6
name: Security Portal42
trigger: tier ≥ T3
tools: Read, Write, Edit, Bash(grep:*), Bash(git diff:*), Bash(git log:*)
---

You are Agent #6 — Security Portal42. ...
[full prompt body]
```

Required frontmatter keys (each on a single line, `key: value`):
- `id` — agent number, integer ≥ 6.
- `name` — short human label, free text.
- `trigger` — when to dispatch (`tier ≥ T3`, `tier ≥ T4+`, `always`, etc.).
- `tools` — comma-separated allowed-tools list (Read, Write, Edit, Bash(...)).

Frontmatter values are read as raw strings — no quoting needed. Do **not**
include colons inside `tools:` values beyond the standard `Bash(grep:*)`
form (the parser splits on the first `:` only).

## Filename convention

`agent-NN-<slug>.md` where `NN` is the zero-padded agent id (so the
directory sorts in numeric order):
- `agent-06-security.md`
- `agent-07-lifecycle.md`
- `agent-08-adversarial.md`
- `agent-09-...md`, etc.

The Step 0c walker globs `agents/agent-*.md` non-recursively. Files in
subdirectories — notably [`upstream/`](upstream/) — are **not** picked up.
Use a subdirectory if you want to keep a prompt as a frozen reference
without dispatching it.

## Body

The body is the prompt text itself, verbatim — no fence, no `**Prompt**:`
marker. The Step 0c walker extracts everything after the second `---`
line; one leading blank line is allowed (and ignored) so the file stays
readable. After that, internal blank lines are preserved as part of the
prompt.

## Adding a new agent

1. Pick the next free id (currently #9 if #6/#7/#8 are taken).
2. Create `agents/agent-NN-<slug>.md` with the frontmatter above.
3. Add a row to the `## Surfaces` table in `review-config.md` if the
   agent owns one or more surfaces.
4. Append a bullet to the `## Custom Agents` pointer list in
   `review-config.md` so future readers can find the new agent.

The canonical doc is `master-review.skill.md` → "Adding a new custom
agent". This README is the format reference; that section is the
process reference.
