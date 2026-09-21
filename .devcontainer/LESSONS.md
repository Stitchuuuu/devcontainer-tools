# LESSONS — project-wide patterns (committed)

> Sister files : `LESSONS.local.md` (gitignored) for personal /
> not-yet-generalisable lessons. Cross-project preferences live in
> `~/.claude/memory/` (auto-memory).

<!-- Entries : one bullet per lesson. Rule first, then *Why* and
     *How to apply* on the same or following line. -->

- **Before editing any `.devcontainer/firewall/*` source file, read
  `.devcontainer/knowledge/firewall.md` end-to-end.** *Why* : the runtime
  config in `/var/run/devcontainer-firewall/` is root-owned and emitted only
  by `init-firewall.sh` at container boot — mid-session edits to
  `domains.txt` / `policy.d/*.yaml` don't refresh dnsmasq / ipset / mitmproxy.
  Running `python3 compile-policy.py` as `node` either fails on `/var/run/`
  writes or recompiles into a file no daemon re-reads, so verifying that way
  is a false signal. *How to apply* : edit the source → the only verification
  path is **rebuilding the devcontainer**. Never claim « tested by recompile »
  without rebuild. Knowledge at `knowledge/firewall.md` L138-188 documents
  the init flow and compile-policy modes.

- **Read `.devcontainer/firewall/policy.d/<host>.yaml` BEFORE any external
  network call (curl / gh / WebFetch / pip / npm) that targets a non-trivial
  host.** *Why* : this repo runs a custom L7 mitmproxy firewall and policies
  restrict paths (e.g. `api.github.com` only allows `/repos/anthropics/*`,
  everything else returns `blocked_path`). A 403 from a public API is far
  more likely a *local* firewall block than a remote-side issue. *How to
  apply* : `ls .devcontainer/firewall/policy.d/` then `cat <host>.yaml` to
  check `endpoints` + `blocked_paths` before retrying. If the target is
  legitimately needed, the right move is editing
  `policy.local.d/<host>.yaml` (gitignored) — not bypassing.

- **Always split commits by *target* : `templates/v2/*` and `.devcontainer/*`
  go in SEPARATE commits, never bundled.** *Why* : this repo's dual-edit
  pattern produces byte-identical changes in `templates/v2/` (the shipped
  template, picked up by future `install.sh` runs of adopting projects) and
  `.devcontainer/` (the dogfood mirror that this repo runs on itself).
  Bundling them obscures intent in `git log` and `git blame` — reviewers
  can't tell whether a hunk shipped to consumers or only patched the
  internal dogfood. Convention surfaced in commits `9fa7d25` / `40116ec`
  and `bb9c7fa` / `0363603`. *How to apply* : commit the `templates/v2/`
  side first (subject prefix `feat(template):`, `fix(template):`, etc.)
  with rollout-plan files riding along ; then commit the matching
  `.devcontainer/` mirror as `chore(dogfood): apply <change> to
  .devcontainer/`. If a single edit spans both — split the staging with
  `git add <path>` per file, never `git add -A`.

- **Devcontainer change order : edit in `.devcontainer/` → verify
  live → mirror byte-identical into `templates/v2/` → ship an
  `updates/<YYYYMMDD-HHMM>-<title>.md` entry (+ optional sibling
  `.patch`).** *Why* : `.devcontainer/` is the live testbed (daemon
  running, can iterate fast), so the change is shaped and validated
  there first. The `templates/v2/` mirror is what downstream FRESH
  installs pull from when a new project runs its first `./install.sh`
  — without the mirror, the change ships nowhere. The `updates/`
  entry is what downstream EXISTING installs pull from — without it,
  every consuming project has to hand-port the change. `install.sh`
  is never run as part of OUR workflow ; it's a downstream-only
  consumption surface. *How to apply* : (1) iterate inside
  `.devcontainer/`, with whatever ad-hoc verification fits (run the
  daemon, eyeball logs, unit tests). (2) Once happy, byte-copy each
  touched file into the matching `templates/v2/` path and `diff` the
  two to confirm zero drift. (3) Per the per-target commit-split rule
  above, commit `templates/v2/` first as `feat(template):` /
  `fix(template):` etc., then `.devcontainer/` as
  `chore(dogfood): apply <change> to .devcontainer/`. (4) Create
  `/workspace/updates/<YYYYMMDD-HHMM>-<title>.md` with a 5-backtick
  fenced `bash` block applying the change
  (inline `sed` for tiny diffs OR `git apply --check && git apply`
  on a sibling `.patch` for larger ones), then `git commit -m` with a
  pre-written message referencing the two upstream commit hashes.
  Skip only when the change is purely local dogfood scaffolding that
  intentionally does not ship — call that out explicitly in the
  session log.

- **Scripts you generate for the user to run on the *host* must use paths
  RELATIVE to the cwd, never `/workspace/...`.** *Why* : `/workspace` is a
  bind-mount point that only exists *inside* the container. On the host the
  same repo lives at an arbitrary path (e.g. `~/dev/myrepo/`,
  `/Users/x/Code/foo/`) that you don't know. Hardcoding `/workspace` makes
  the script fail with `cd: No such file or directory` on every host. *How
  to apply* : start with `#!/usr/bin/env bash` + `set -euo pipefail`, a
  header comment "run from repo root on host", and every path
  `./relative/...` or `$(pwd)/...`. Files dropped in `.tmp/foo/` on the
  host are then visible at `/workspace/.tmp/foo/` from inside the
  container via the bind mount.

- **Targeted updates ship as `updates/<YYYYMMDD-HHMM>-<title>/` folders
  containing exactly two files : `update.patch` + `update.md`.
  Downstream projects fetch them via sparse-checkout into
  `.tmp/devcontainer-updates/` — never into `.tmp/upgrade-v2/updates/`
  (that path was the old convention, retired June 2026).** *Why* :
  one folder per fix keeps `git log` / archival surgical, the
  sparse-checkout target is ~1 MB vs ~30 MB for the full-release
  clone used by the version-bump flow, and the `.tmp/devcontainer-updates/`
  prefix is short enough to type / paste without abbreviation. The
  bootstrap + per-update flow is documented in the "Targeted updates"
  section of [UPGRADE-v2.md](../UPGRADE-v2.md#targeted-updates-updatesname).
  *How to apply* : when shipping a new targeted fix, create
  `updates/<ts>-<title>/` with `update.patch` (the diff) and
  `update.md` (the recipe). Every path inside the recipe's bash
  blocks references `.tmp/devcontainer-updates/updates/<ts>-<title>/update.patch`.

- **`updates/*/update.md` bash blocks must be flat-pasteable in an
  interactive zsh — no `set -euo pipefail`, no `# …` step comments, no
  leading variable assignments, no multi-line `if/then/fi`.** *Why* : the
  user pastes these recipes line-by-line (or block-by-block) into the host
  terminal — never through `bash <<EOF`. `set -euo pipefail` in an
  interactive shell kills the whole session on the first unset var or
  non-zero exit ; `# 1.` / `# 2.` step headers and `PATCH=…` declarations
  either pollute history or fail to survive across multi-paste sessions.
  This rule was paid for by [updates/20260613-1934-notify-accents-state/update.md](../updates/20260613-1934-notify-accents-state/update.md)
  — the user had to manually rewrite it ; **DO NOT regenerate that shape**.
  *How to apply* : when authoring `updates/<YYYYMMDD-HHMM>-<title>/update.md`,
  model the Apply / Rollback blocks on the canonical
  [updates/20260613-0929-plus-button-chrome/update.md](../updates/20260613-0929-plus-button-chrome/update.md)
  — bare commands separated by blank lines, paths inlined (no `$VAR`,
  always `.tmp/devcontainer-updates/updates/<name>/update.patch` written
  in full), multi-line commands joined with `\` continuations,
  multi-statement conditionals folded onto one line (`if …; then …; fi`).
  If a recipe genuinely needs `set -e` semantics, ship a sibling `.sh`
  file and have the recipe call `bash ./that.sh` — never inline the guards
  in the `.md`.

- **Commit messages stay short and self-contained — never append
  `— apply updates/<ts>-<title>` or any other rollout/tracker suffix.**
  *Why* : `git log` / `git blame` / PR diffs are read without the
  rollout doc open. A suffix like `— apply updates/20260613-1934-notify-accents-state`
  bloats the subject past the 50-char readable budget, decays the
  moment the `updates/` entry is archived, and adds zero information
  the diff doesn't already carry. The subject must describe **the
  change itself** (`fix(notify): accents decode + state payload`),
  not the delivery vehicle. Reinforces §10 of
  [CLAUDE-dev.md](claude/CLAUDE-dev.md). *How to apply* : when
  writing a `git commit -m` line inside an `updates/*/update.md`
  recipe, in a session prompt, or anywhere else — stop at the change
  description ; never tack on `— apply updates/…`, `— rollout step N`,
  `— plan abc123`, etc. If a reviewer needs the rollout context, the
  PR description / commit body is where it goes, not the subject.

- **When proposing a commit via `AskUserQuestion` (per §10 CLAUDE.md +
  [feedback-commit-validation-askuser](../.claude/memory/feedback-commit-validation-askuser.md)),
  print the FULL commit subject + body AND the file-by-file staged list
  (`git diff --cached --stat` or equivalent) in the chat BEFORE the AskUser
  call.** *Why* : the AskUserQuestion preview is a secondary panel the
  user has to focus on to read ; a terse chat teaser ("commit shape
  looks like X, 2 files touched, +9/-9") does not give enough signal to
  approve without expanding the preview, and the user has repeatedly
  asked for the *real* commit message + files-changed list inlined.
  Chat text stays in the transcript ; the preview can be missed. *How
  to apply* : right before `AskUserQuestion`, emit (a) the exact
  commit message inside a fenced code block, and (b) an explicit staged
  list with per-file additions / deletions. THEN call AskUserQuestion
  with the Commit / Modifier / Annuler triple ; the preview may
  duplicate the same content but the chat block is what the user
  actually reads. Applies to every commit — session bookkeeping, hot
  fixes, docs-only, all of them.

- **Before searching the web for tooling docs, check
  `.devcontainer/knowledge/<tool>.md` first.** *Why* : this repo ships
  cheat-sheets for every dev-tool baked into the base image — `wtf.md`,
  `firewall.md`, `docker-base-image.md`, `ollama-local.md`,
  `extension-points.md`. They're compiled from the tool's own source and
  refreshed when the base image bumps. Reaching for WebFetch / gh api
  instead means (a) burning permission prompts on `Bash(gh:*)` /
  `Bash(wtf:*)`, (b) chasing 404s on stale doc-site URLs, and (c)
  duplicating the exact schema already spelled out in the knowledge file.
  Real hit : spent ~5 tool calls fetching wtfcmd docs online for the
  `is_array` variadic + `cwd:` + `--` passthrough syntax — every field
  was in `knowledge/wtf.md` already. *How to apply* : first move on any
  « how does `<X>` work » question is `ls .devcontainer/knowledge/` +
  `Read` the matching file. The « Canonical links » footer at the end
  of each cheat-sheet is the escape hatch when the local doc feels
  stale ; the doc itself is the primary source.

- **A persisted flag is a symptom, not a diagnosis — always co-locate
  the discriminant that tells you WHY the flag is set.** *Why* : the
  purrpause b.18 anti-bypass rule fired on `popup_pending == true`
  at cold-boot as a single-signal proxy for "kid killed the service".
  But `popup_pending == true` at cold-boot has TWO valid causes —
  malicious kill (unclean shutdown) AND legitimate PC restart during
  a popup (clean shutdown) — with opposite required responses (punish
  vs resurrect). Collapsing them punished legitimate users. Same
  pattern applies to any "did the last shutdown crash mid-operation"
  flag : the flag records the state, `was_clean_shutdown` records the
  cause. *How to apply* : whenever a design uses a boolean flag from
  runtime.dat / any persisted state as a proxy for intent, ask "what's
  the second orthogonal signal that disambiguates the two possible
  causes ?" — if there isn't one, the design is ambiguous. Thread
  both signals into the pure-kernel inputs (in purrpause's case :
  `ResolveInputs.was_clean_shutdown` alongside `popup_pending`) so
  the decision logic is fully expressed without caller-side gates.

- **On a Parallels-NATed link (Shared network), a Windows Firewall
  inbound *drop* can surface on the macOS client as `No route to
  host` (EHOSTUNREACH), not the usual silent timeout.** *Why* : during
  a purrpause LAN-console smoke, the Mac got `nc: ... No route to host`
  hitting the guest's `0.0.0.0:8787`. That error normally means an
  L2/ARP/routing failure, so ~5 rounds were spent chasing the Parallels
  network (bridged vs shared, ARP tables, vnic/bridge interfaces) — when
  the real cause was the guest's **missing inbound firewall rule**
  (Private profile, default-deny). The Parallels NAT gateway relayed the
  firewall drop back as an ICMP unreachable → EHOSTUNREACH on the client.
  *How to apply* : when a guest service listens on `0.0.0.0` and answers
  on `127.0.0.1` but not from the host, **do the firewall-off/on A-B test
  early** (`Set-NetFirewallProfile -All -Enabled False`, retest, re-enable)
  — it's decisive in one step. Don't let `No route to host` alone rule out
  the firewall on a virtualized/NATed link ; the error class is unreliable
  there. Verify firewall rules in an **elevated** shell — non-admin
  `Get-NetFirewallRule`/`Get-NetFirewallPortFilter` return empty or
  Access-Denied and give false "no rule" reads.

- **`cargo clippy` on Linux does NOT lint `#[cfg(windows)]` modules —
  run `cargo xwin clippy --target x86_64-pc-windows-msvc` before
  committing purrpause code that touches Windows-only files.** *Why* :
  in purrpause, whole modules are `#[cfg(windows)]` (`modes/config/{tabs,app}`,
  `platform/win32/*`), so they simply don't compile on the Linux dev host —
  Linux clippy reports them clean even when they carry real lints. A session-7
  change pushed `ui_security` to 8 args (clippy `too_many_arguments`, limit 7) ;
  the Linux clippy in the DoD saw zero warnings, and only the cross-compiled
  clippy caught it — against the project's established zero-warnings-on-Windows
  bar. *How to apply* : for any change under a `#[cfg(windows)]` module, add a
  `cargo xwin clippy --bin SystemHealthAgent --target x86_64-pc-windows-msvc`
  pass (aarch64 also works) to verification, not just `cargo clippy` on Linux.
  The aarch64 `pack` build proves it *compiles* but does not run clippy.

- **A heredoc that must expand variables cannot carry backticks — not even
  inside a comment.** *Why* : `<<EOF` (unquoted) is a full expansion context,
  so backticks are command substitution wherever they appear. In
  `init-firewall.sh`, the heredoc writing the dnsmasq injections held a
  comment reading « no manual [ipset add] workaround needed », the two words
  in backticks : every container boot ran `ipset add` with no arguments **as
  root**, printed `ipset v7.17: Missing mandatory argument` to stderr, and
  spliced the empty output into the generated conf, truncating the comment.
  It shipped in the image for three sessions because the firewall worked
  anyway. The same mistake reappeared the same day in a preflight script,
  where a comment backticked two shell builtins and ran them. Markdown habits
  — backticks around identifiers — are exactly what triggers it. *How to
  apply* : when writing a heredoc, pick the delimiter deliberately. `<<'EOF'`
  (quoted) if nothing needs expanding — then backticks are safe. `<<EOF` only
  when a variable must expand, and then **no backticks in the body**, prose
  included ; write `ipset-add` or plain words instead. To audit a file, slice
  its heredocs with `awk '/<<EOF/,/^EOF$/' <file>` and grep the result for a
  backtick — any hit is a command substitution waiting to fire.

- **Brace every `$var` that a non-ASCII character follows : `"${p}…"`, never
  `"$p…"`.** *Why* : host-side scripts run under macOS's bash **3.2**, which
  is not multibyte-safe when parsing identifiers — it absorbs the UTF-8 bytes
  of `…` / `—` / `→` into the variable name and looks up `p\xe2\x80\xa6`.
  Under `set -u` that is a hard `unbound variable` abort, and the error names
  a mangled variable that appears nowhere in the source, so it reads as
  corruption rather than a parsing rule. A preflight run died this way right
  before its most expensive phase, on `step "devc-hook $p…"`. Our scripts are
  full of `…`, `—` and `→` in user-facing strings, so the pattern is common.
  *How to apply* : brace by default in any string mixing a variable with
  typographic punctuation. Sweep a script with
  `grep -nP '\$[A-Za-z_][A-Za-z0-9_]*[^\x00-\x7F]' <file>` — no `\{?`/`\}?`
  in that pattern on purpose, adding them matches the already-braced (safe)
  form too. Comments that merely mention the bug still show up ; read the
  hits, don't count them. `bash -n` does **not** catch this — the failure is
  at expansion time, so a script can be syntactically perfect and still die
  mid-run.

- **A new assertion in `packages/devcontainer-sandbox/test/` is not delivered
  until it is in [`TESTING.md`](../packages/devcontainer-sandbox/TESTING.md).**
  *Why* : the suites are the contract of a **published** image, and the people
  who need to know what is guaranteed — whoever pulls the image, whoever signs
  off a release — do not read bash. A test that exists only in the source is a
  guarantee nobody outside the repo can see. The catalogue also drove out two
  real defects during 4.1b-bis, simply by forcing each label to be explained in
  plain language : a stale label naming `jq` after `jq` had left that code
  path, and a group of assertions that turned out to be untestable. *How to
  apply* : same commit as the test, never a follow-up. One row per assertion,
  two columns — what it guarantees for a non-technical reader, then the
  mechanism for a dev. Group only assertions that differ by one parameter, and
  then name every value in a sub-table. Keep the left column **byte-identical**
  to the label the suite prints : that is how someone greps from a red run back
  to the explanation. Cross-check with the runtime output, not the source —
  loops emit several assertions per line of code.

- **Never run a project-wide `docker compose` command against the devcontainer
  stack — scope it to the service you actually mean.** `down`, `up -d`,
  `restart` and `stop` with no service argument act on **every** service,
  including `app` — the container the human is working in. *Why* : killing
  `app` destroys the shell history still buffered in every open terminal. The
  `/commandhistory` volume survives, but history is only flushed when a shell
  exits cleanly, so an abrupt stop loses whatever was typed since the terminal
  opened. This cost the user their history on 2026-08-24, from a script whose
  job was to restart **dind alone** and which opened with
  `docker compose down --remove-orphans`. And `down -v` is worse still: it
  deletes every *managed* named volume — `claude-config`
  (`/home/node/.claude`, **the Claude session transcripts**), `bash-history`,
  and dind's image store. Only `external: true` volumes survive, which is the
  one reason the credentials did. *How to apply* : name the service —
  `docker compose up -d dind`, `docker compose rm -sf dind`. Before writing any
  compose command into a script or a handoff, ask which services it touches,
  and put the answer in a comment. When a change genuinely requires `app` to be
  recreated (a `Dockerfile` or `devcontainer.json` edit), do not do it as a
  side effect — say so and let the user pick the moment. When a container will
  not start, diagnose before destroying: the usual cause is a volume **name**
  mismatch against an `external: true` declaration, and
  `docker compose config --format json` prints the names compose actually
  wants.

- **One browser-driving command per Bash call — never as one half of `a && b`.** *Why* : two reasons, and neither is the window flashing (that is gone since captures rasterise with `fromSurface: false` and nothing calls `Target.activateTarget`). First, these commands still **navigate the human's tab** away from whatever was on screen ; in a compound call the tool description names only one of the two, so the human reads "compose an A/B image" and their page changes underneath with nothing tying the two together. Second, `cdp.mjs` now takes a **browser lock** : there is one tab and every tool navigates it, so the second half of a compound call is simply **refused with exit 10** while the first still holds it. *How to apply* : one `wtf claude-live *` per Bash call, with a description that says "drives the browser" in those words. Batch the reads instead of repeating the call — one `probe` with ten `--sel` is one navigation and one lock acquisition, ten calls are ten of each. The offline half (`wtf claude-script`) has no such constraint and can be chained freely.

- **A `wtf claude-live` failure is usually the world not being ready, not a bug — read the exit code before debugging.** *Why* : three preconditions live outside the container and each has its own code. **3** = the debug Chromium is not running, which only the human can fix (`wtf browser` is HOST ONLY — it needs a window) ; retrying it from in here, or trying to launch it, just produces a second confusing failure. **9** = the app's tab is not frontmost in its window, so it has stopped painting and any capture would be a frozen frame measured as if it were live — note the *window* may sit behind the editor, and `document.hasFocus() === false` is the normal supported state, so "click the window" is the wrong fix and the message says so. **10** = another session holds the browser. *How to apply* : relay the message's own `→` lines to the human as a copy-pasteable command (markdown link for any path), then **wait**. Never loop a retry. For 9, `wtf claude-live front` positions the tab from here ; for 10, re-run the same command after the holder finishes, or pass `--wait-lock <seconds>` to block instead of failing.

- **A subagent worktree branches from an ANCESTOR, not from your HEAD — an agent you spawn sees older code than you do.** *Why* : `Agent` with `isolation: "worktree"` creates its tree from the repository's base, not from the branch the parent session is on. Measured on a three-agent smoke test : every worktree was rooted at an ancestor of HEAD, and the agents reported file counts and config entries that were honest descriptions of *their* trees and wrong for the branch tip. Neither the parent nor the agent has any reason to notice. *How to apply* : for parallel work on a feature branch, either state the base commit in the prompt and have the agent verify it (`git log --oneline -1`), or give the agent absolute `/workspace` paths for anything it must read at your revision. Treat a factual claim about repository contents from a worktree agent as a claim about ITS tree — check it against yours before acting, as §7 of CLAUDE-dev.md already requires.

- **A subagent worktree is not a jail : absolute paths still reach the shared workspace.** *Why* : the isolation covers what an agent deliberately writes in its own tree, not what it can read or touch elsewhere. Observed in the same smoke test : an agent asked to read a file absent from its stale worktree returned the correct answer anyway, because it went and read `/workspace`. Useful here, dangerous at scale : two agents that resolve an absolute path can still write over each other, and the worktree gives a false sense of a sandbox. *How to apply* : never rely on the worktree to enforce file ownership between parallel agents — say in each prompt which files it owns, exactly as [`wave-run.sh`](claude/scripts/wave-run.sh)'s STAY IN YOUR LANE clause does. If a read must come from the parent's revision, ask for `/workspace/...` explicitly, so the crossing is intentional and visible rather than an accident of resolution.

- **A prompt that names a repository the agent is not standing in will be obeyed over its own `cwd`.** *Why* : a verifier spawned with `cwd` set to a unit's worktree, but whose prompt opened with « Repository : <the shared workspace> », believed the prompt. It went to the workspace — a checkout carrying neither the change under test nor its commit — ran the project gate *there*, and reported the count it got as a contradiction of a report that was exact to the test. A human was then asked to arbitrate a false accusation. *How to apply* : name the `cwd` and nothing else, and say it is the ONLY tree to judge from — `--dangerously-skip-permissions` reaches any absolute path, so the prompt is the only fence there is. Hand the agent the mechanical facts already established instead of leaving it to go and re-establish them, and put that check out of its remit in words. Third of this family, after « a subagent worktree branches from an ANCESTOR » and « a subagent worktree is not a jail » : the tree an agent reads is never automatically the tree you meant.

- **Keeping "the last N chars" of a command's output is not keeping evidence — it keeps whatever the command happens to print last.** *Why* : a runner storing 2000 chars as `output_tail` recorded a red gate as ~2,3 Ko of expected devcontainer skip lines (« API unreachable… », « S3_* not configured… ») followed by `GATE BLOCKED` — because [gate.mjs](claude/scripts/gate.mjs) prints those skips **last**. The `LINT` / `TEST` totals and the whole `FAIL <file>` block, the only lines that say *what* was red, fell off the front. A human was then asked to arbitrate « report claims green, recording says red » with nothing in the run able to answer it. *How to apply* : persist the full output beside the log (`logs/<unit>.gate.log`) and demote the tail to a preview ; where a cap is unavoidable, keep **head + tail** and cap in lines, not bytes. Before trusting any `_tail` field, run the command once and look at what its last N chars actually contain — if it is the part you could already predict, the record is worth nothing.

- **A regex that parses a model's answer must accept the answer's formatting range — bold, heading, spacing — or make « matched nothing » a loud state of its own.** *Why* : a verdict parser anchored on `/^\s*VERDICT/` while the prompt asked for « a line `VERDICT: confirmed` » ; a verifier that answered `**VERDICT: contradicted**` — bold, the most common markdown reflex there is — matched nothing, and the no-match path scored as *confirmed*. The report on disk said contradicted, the journal line one second later said confirmed, and the work merged on that answer. Found only by an audit diffing every report against every journal line — nothing in the run surfaced it. *How to apply* : when a prompt asks for a magic line, either tolerate emphasis and heading markers around the anchor (`[\s*_#]*`), or route « the magic line is absent » to its own explicit outcome — never to the benign default. The default a non-match falls into is the whole risk : arrange the mapping so the *dangerous* reading is the one that needs no parsing luck. Same family as « keeping the last N chars » : a reader that silently keeps the wrong thing reports the same answer whether or not the thing happened.

- **Any manual test procedure must name the account to log in with — email AND password, on the page it applies to.** *Why* : a checklist that opens with « drop `mock-happy.pdf` on `/data` » assumes the human is already authenticated as the right role, which is exactly what a fresh container, a cleared cookie jar or an `e2e` run (it sends `Storage.clearCookies` browser-wide) undoes. They then land on the login screen with a checklist that never mentions one, and the first step of every procedure becomes "go and find the credentials". Worse, a role-gated page shows empty or forbidden for the wrong seeded user, which reads as a regression. *How to apply* : put the account at the TOP of the procedure, before the first action, as `email / password` in full. Applies to anything the human executes by hand : TEST-PLANs, repro steps, « here is what is left for you to check » messages. If a procedure spans two apps or two roles, name the account again at the step where it changes.

- **A lifecycle hook has no proxy : in strict mode it cannot reach the network unless it sources one itself.** *Why* : strict mode routes every egress through mitmproxy, and the `HTTP_PROXY`/`HTTPS_PROXY` that say so are exported by `/etc/profile.d/devcontainer-proxy.sh` — which only a **login shell** sources. `devc-hook` fragments are not login shells, so a `curl` inside one fails at the *connection*, before any HTTP status exists. The symptom is maximally misleading : the banner says « could not fetch » with no code, the same command run by hand in `docker exec ... bash -lc` works every time, and it therefore reads as a startup race. It is not — it is deterministic, and adding `--retry` does nothing. Found on `45-ext-patches.sh`, which silently booted a container with an unpatched extension ; `45-claude-update-probe.sh` has the same blind spot and nobody noticed because it is silent on failure by design. *How to apply* : any hook fragment doing network I/O must, before its first request, source `/etc/profile.d/devcontainer-proxy.sh` when `HTTPS_PROXY` is unset. Test a network hook by running the **phase** (`devc-hook on-create`), never by running the command in a shell — a login shell hides exactly this bug.

- **A build ARG feeding only a LABEL still invalidates everything below it — put labels last.** *Why* : `ARG BASE_VERSION` sat at Dockerfile line 35 and fed a `LABEL` two lines later, above 39 `RUN`/`COPY` steps including a 243 MB download. Changing the version rebuilt the entire image to write a metadata string, and the release process requires that bump every time. It also split the cache between people : a build that forgot `--build-arg BASE_VERSION` took the `0.0.0-dev` default and shared nothing with one that passed the real value, which reads as "the cache is broken" rather than "these are two different builds". Measured after moving the block to the end : 57 s → 0,33 s, same labels. *How to apply* : `LABEL` and the `ARG`s that feed only labels go at the **bottom** of a Dockerfile, after the last `RUN`/`COPY`. Re-declare the `ARG` there — the earlier declaration is out of scope. More generally, before adding an `ARG`, ask what it invalidates: anything that does not change the filesystem belongs below everything that does.

- **A flag documented in a usage header but absent from the argument parser
  does not do nothing — it does whatever the script does by default.** *Why* :
  `ext-patches-sync` announced `--status  say what is configured and cached, do
  nothing` on line 5 of its own header, from its first version, and the script
  had **no argument parsing at all**. So the one flag whose entire promise is
  "change nothing" fell straight through to the apply path and rewrote the
  extension — measured, with the defect reinstated as a counter-proof: the
  throwaway bundle went from 8 bytes to 31. Nobody noticed because every suite
  that mentioned the binary asserted it *exists* and is executable, and none
  asserted what it *does*. A usage header is documentation people trust more
  than code, precisely because it sits inside the code. *How to apply* : when a
  script grows its first flag, parse `"$@"` **and** refuse an unknown option
  (`exit 64`) in the same commit — ignoring a flag is how one comes to mean its
  opposite. Before trusting a flag you did not write, `grep` the script for
  `$1`/`getopts`/`case "$@"` and check it is read at all. And when a binary is
  only covered by "it is baked and executable", treat that as *no* coverage:
  make the last hardcoded path in it an env seam (`RESTORE_EXT_PATCHES`, next
  to the `TOOLKIT_DIR`/`BUILD_ENV`/`DEVC_CONFIG_DIR` that were already there)
  so its behaviour can be driven from a suite at all.

- **A patcher marked `@patch-critical` was critical against a version of the
  *host*, not against the eternity — re-measure before letting it block a
  release.** *Why* : `navigator-pending-migration-fix` carries a real stack
  trace showing `PendingMigrationError` aborting activation, and the README
  still says the accessor "throws on any access". On the VS Code actually in
  use (1.127.0) it does not: `extensionHostProcess.js` installs a getter that
  calls `onUnexpectedExternalError`, whose default handler throws inside a
  `setTimeout` — asynchronously, so it never reaches the caller — and the
  getter, having no `return`, yields `undefined` synchronously. The module load
  completes. The claim was true when it was written, on CC 2.1.145 and an older
  VS Code, and it silently became a release blocker for an image that ships the
  extension unpatched. *How to apply* : a criticality marker needs the host
  version it was measured against written next to it, and a claim about
  upstream behaviour ("it throws") is a measurement with an expiry date, not a
  property. Before treating one as blocking, read the current bundle — the
  answer is usually thirty seconds of `grep` in `extensionHostProcess.js`, and
  it is cheaper than the release decision that hangs on it.

- **Un banc qui ne prouve pas qu'il a provoqué quelque chose ne mesure rien.**
  *Why* : `bare-check.sh --login` invalidait l'`accessToken` pour forcer un
  `authentication_failed`, et laissait le `refreshToken` valide à côté, avec
  `refreshTokenExpiresAt` dans le futur. Claude Code prenait le 401, partait
  sur le chemin de refresh, se ré-authentifiait — la session restait connectée
  et le banc posait quand même ses questions. Toute une session de banc a été
  perdue avant que l'utilisateur ne dise « je peux pas trigger le unauth ».
  La garde humaine existait pourtant (« l'écran apparaît-il ? ») et n'a pas
  protégé : on répond oui à une question qu'on n'a pas les moyens de vérifier.
  *How to apply* : tout banc qui provoque un état doit **affirmer par machine
  qu'il l'a atteint** avant de demander quoi que ce soit à un humain — ici une
  vraie requête (`claude -p`), parce que `claude auth status` rapporte
  `loggedIn: true` sur les dates d'expiration du fichier, jamais sur la
  validité du jeton. Et quand on invalide un identifiant, invalider **toutes
  les voies de récupération**, pas la première.

- **Ancrer sur ce qui survit, pas sur ce qui est à côté.** *Why* : les cinq
  ré-ancrages de 2.1.268 ont tous la même forme. `model-mode-affinity` était
  ancré sur la tête d'une chaîne de virgules qu'upstream a réécrite, alors que
  la queue où il injecte est byte-identique depuis 2.1.220. `model-selection-fix`
  recopiait le préambule de `loadUserSettings` uniquement pour en extraire les
  idents `fs`/`path` — 268 a scindé la méthode et le patch est mort pour un
  corps auquel il ne touchait même pas. `user-action-observer` épelait
  `JSON.stringify` alors qu'il se moque de qui sérialise.
  *How to apply* : avant d'écrire une regex, demander « de quoi ai-je
  réellement besoin ? ». Capturer plutôt qu'épeler (le sérialiseur, le module,
  le nom de classe) ; tolérer les arguments en queue plutôt que compter les
  paramètres ; préférer un site **unique dans tout le bundle**
  (`setPreferredLocation("panel")` : exactement une occurrence sur 220, 258 et
  268) à un site voisin plus lisible ; et injecter un prologue plutôt que
  recopier le code d'upstream pour le réémettre.

- **Un sous-correctif qu'upstream a rattrapé se retire sur ce que dit le
  bundle, pas sur un numéro de version.** *Why* : sur 2.1.268 l'étape 4 de
  `icon-fix-open-in-current-panel` est devenue inutile — `d1$() =
  Si$()?.viewColumn ?? ViewColumn.Active` fait ce que notre injection faisait,
  en mieux (repli sur le premier groupe d'onglets éligible). Une garde
  `if version >= 2.1.268` aurait créé deux chemins de code à maintenir.
  *How to apply* : tester la **forme lue** (« le troisième argument est-il
  encore `ViewColumn.Active` ? ») et se retirer avec un message qui le dit.
  Un seul chemin de code, et il reste juste quand upstream change d'avis.

- **Un artefact qui nomme sa version cible doit être refusé, ou au minimum
  signalé, quand on l'applique ailleurs.** *Why* : le schéma de tag
  `cc<version>-r<n>` du dépôt de patchers DÉCLARE l'extension pour laquelle il
  a été coupé, et `ext-patches-sync` ne lisait ce nom que comme clé de cache,
  ligne de statut et URL de fetch — jamais comme une affirmation. Un pin resté
  en arrière après quatre bumps CC (`cc2.1.258-r2`) a donc été appliqué tel
  quel à une extension 2.1.272 : huit patchers ont échoué bruyamment et
  **neuf ont réussi**. Ce sont les neuf le problème — une ancre de 2.1.258 qui
  matche encore dans du code 2.1.272 n'est pas une ancre qui veut encore dire
  la même chose, et `node --check` prouve seulement que le résultat parse. Le
  seul garde-fou existant, `warn_untested_version`, comparait le mauvais
  référentiel (le `versions.json` qui **voyage avec le jeu téléchargé**, donc
  d'époque) et se noyait sous huit lignes rouges.
  *How to apply* : quand un nom d'artefact encode sa cible — tag, dossier,
  digest — lire cette cible et la comparer à la réalité, au lieu de traiter le
  nom comme opaque. Sortir l'alerte **en dernier**, après ce qu'elle qualifie,
  et la répéter sur le chemin du court-circuit : « déjà appliqué » est
  précisément la phrase qui ne doit jamais rester seule quand le jeu appliqué
  était pour une autre version.

- **Une assertion ne doit jamais grep une chaîne que le correctif lui-même
  cite.** *Why* : la garde de capability de `init-firewall.sh` explique que
  iptables ment en disant « you must be root ». L'assertion censée prouver que
  ce diagnostic opaque n'apparaît plus grepait… `you must be root`, et matchait
  donc le texte du refus. Elle échouait alors que la garde marchait
  parfaitement. Corrigée en `Could not fetch rule set generation id`, la
  signature propre d'iptables, que rien d'autre n'émet.
  *How to apply* : ancrer une assertion de non-régression sur une chaîne que
  **seul le défaut** peut produire. Si le message de correction cite le
  symptôme — ce qui est souvent la bonne rédaction — la chaîne citée est
  disqualifiée comme sonde.

- **Une suite qui lit une variable d'environnement doit la neutraliser à
  l'entrée.** *Why* : `ext-patches-sync` préfère l'environnement au `.env`
  (compose injecte à la création), donc `toolkit.test.sh` héritait du pin, du
  dépôt et du token du container qui l'exécutait : 69/0 sur le host, **5
  échecs** dans le devcontainer de dogfood, même code, même commit. La suite
  répondait sur le container au lieu de répondre sur sa fixture.
  *How to apply* : `unset` en tête de suite toute variable que le code sous
  test lit depuis l'environnement, et la poser explicitement dans les seuls
  tests qui l'éprouvent — le précédent est `run-firewall-suites.sh:26-29` avec
  `FIREWALL_ALLOW_LOCAL_AT_REBUILD`.

- **`npm pack` jette silencieusement tout fichier nommé `.gitignore`.** *Why* :
  le scaffold de `devc init` embarque un `.devcontainer/.gitignore` dans
  `packages/devcontainer-cli/templates/` ; le checkout l'avait, tous les tests
  passaient, le tarball ne le contenait pas — 24 fichiers sur 25. Le plan
  étant construit en parcourant le dossier, une installation depuis le
  registre aurait scaffoldé un projet **sans `.gitignore`**, sans erreur.
  Trouvé par la passe de vérification, jamais par le checkout de dev.
  *How to apply* : nommer le template `_gitignore` et le renommer à l'écriture
  (convention create-vite) ; et garder un test qui compare `npm pack
  --dry-run --json` au parcours du dossier — c'est le seul endroit où le
  défaut est visible.

- **`npx <paquet> <cmd>` sans spec de version ne lance pas la copie locale.**
  *Why* : dans `libnpmexec` (npm 8-11), un spec sans version est un *tag* ; la
  branche appelle `getManifest` (`preferOnline`) à chaque exécution, compare
  l'URL du tarball résolu à celle du `node_modules` local, et **installe puis
  lance la version plus récente** dès qu'il y en a une au registre. Un
  `devDependencies` + lockfile ne protège donc rien sous cette forme. Un spec
  en **plage** (`@0.x`) prend l'autre branche : arbre local interrogé,
  `semver.satisfies`, copie locale lancée sans réseau — prouvé par un test
  contre un registre mort (`test/npx-resolution.test.ts`).
  *How to apply* : toute commande répétée qui passe par `npx` (un
  `initializeCommand`, un hook) porte une plage ou une version exacte, jamais
  le nom nu ; et éviter `^` (échappement cmd.exe) et `>` (redirection) dans la
  chaîne — `0.x` n'a aucun métacaractère.

- **Ne jamais passer un chemin absolu en premier argument positionnel de
  `npx`.** *Why* : avant de résoudre un paquet, libnpmexec teste si la
  commande demandée existe déjà en bin local, avec
  `resolve(<dir>/node_modules/.bin, args[0])`
  (`libnpmexec/lib/file-exists.js`). Quand `args[0]` est absolu, `resolve`
  jette le répertoire et rend le chemin lui-même — qui existe. npx en conclut
  que la commande est installée, **n'installe rien**, n'atteint jamais
  `getBinFromManifest` (qui aurait traduit le spec en nom de bin) et passe le
  fichier à `sh` : `Permission denied` sur un `.tgz`, sans un mot sur
  l'installation manquante. *How to apply* : pour jouer un tarball local,
  `npx --yes --package /chemin/x.tgz -- <bin> <args>`. Un chemin **relatif**
  marche aussi par accident (`resolve` ne court-circuite pas), mais la forme
  `--package` est la seule qui dit ce qu'elle fait.

- **Un npm imbriqué hérite des `npm_config_*` du npm parent.** *Why* :
  `npm publish --dry-run` exporte `npm_config_dry_run=true` ; le `npm pack`
  lancé par un test sous `prepublishOnly` l'hérite, **sort en 0 et n'écrit
  aucun tarball**. Le test casse alors sur son propre fixture
  (`assert.ok(tarball !== undefined)`) et l'échec accuse le test, pas la
  cause. Vaut pour `dry_run`, `registry`, `cache`, `production`… *How to
  apply* : tout `spawn('npm'|'npx', …)` dans un test part avec un
  environnement nettoyé des `npm_config_*` qui changeraient son sens — et le
  commentaire dit lequel, sinon le prochain le remet.

- **Une commande shell écrite et vérifiée en bash n'est pas vérifiée pour un
  lecteur en zsh.** *Why* : zsh ne découpe pas une expansion de variable non
  quotée en mots. `DEVC="node /x/devc.mjs"` puis `$DEVC init` marche en bash
  et échoue en zsh sur `no such file or directory: node /x/devc.mjs` — le
  nom de commande est la chaîne entière. Un guide rédigé depuis ce container
  (bash) et joué sur le Mac (zsh) tombe dessus à la première ligne, et le
  script qui porte la même forme en interne, lui, marche — ce qui égare le
  diagnostic. *How to apply* : dans une doc destinée à être collée dans un
  terminal, un lanceur est une **fonction** (`devc() { node "$X" "$@"; }`),
  jamais une variable. Dépannage sans réécrire : `${=VAR}`.

- **`return <promesse>` dans un `try/finally` exécute le `finally` avant que
  la promesse ne se résolve** — et si ce `finally` ferme une ressource, la
  promesse ne se résout jamais. *Why* : mesuré sur `devc init`, où le chemin
  « projet déjà scaffoldé » faisait `return reportExisting(wizard)` sans
  `await` dans un `try { … } finally { readline?.close() }`. L'interface
  readline était créée pendant l'appel puis fermée aussitôt, question
  pendante : `rl.question()` ne se règle plus, la boucle d'événements se
  vide et node sort en **13** avec `Warning: Detected unsettled top-level
  await`. À l'écran, le prompt s'affiche et le shell reprend la main sans
  rien lire. *How to apply* : dans un `try` qui a un `finally` libérant
  quoi que ce soit, écrire `return await f()`, jamais `return f()` — la
  règle vaut aussi quand le `finally` ne fait « que » du log.

- **Une couture d'injection testable peut rendre un défaut de cycle de vie
  invisible à toute la suite.** *Why* : les 129 tests de `devc init`
  passaient `options.ask`, donc `readlineAsk()` n'était **jamais** construit
  et le `close()` du `finally` était un no-op — le défaut ci-dessus ne
  pouvait pas apparaître. La couture qui rend les prompts testables est
  exactement celle qui masque la ressource qu'ils tiennent. *How to apply* :
  quand une dépendance est injectable, garder **un** test qui prend le
  chemin réel (ici : un `PassThrough` avec `isTTY = true` et pas de `ask`,
  sous `{ timeout }` pour que l'échec soit une expiration lisible et non un
  blocage). Le contrôle que le test sert à quelque chose se fait en cassant
  le correctif dans le **compilé** et en vérifiant qu'il rougit.

- **`node_modules/` est partagé entre le Mac et le container par le
  bind-mount, et un paquet à binaire natif par plateforme ne peut pas
  satisfaire les deux.** *Why* : `typescript@^7.0.2` livre son compilateur
  en optionalDependencies par plateforme
  (`@typescript/typescript-darwin-arm64`, `…-linux-arm64`, …). Le
  `npm install` du Mac ne pose que la brique darwin ; dans le container,
  `npm run build` meurt sur `Unable to resolve
  @typescript/typescript-linux-arm64`. Pire : `npm install` de la brique
  linux **supprime** la darwin (« added 1 package, and removed 1 package »),
  donc réparer un côté casse l'autre, et npm refuse la darwin sur linux
  (`notsup Actual cpu`). *How to apply* : pour faire coexister les deux,
  récupérer le tarball et l'extraire à la main —
  `npm pack @typescript/typescript-<os>-<arch>@<version>` puis
  `tar xzf … --strip-components=1 -C node_modules/@typescript/<nom>`. Et
  s'en souvenir au moment de choisir une dépendance : un paquet à binaires
  par plateforme coûte cette gymnastique à chaque fois
  ([.claude/rules/dependencies.md](../.claude/rules/dependencies.md)).
