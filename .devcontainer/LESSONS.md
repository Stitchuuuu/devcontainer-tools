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
  bootstrap + per-update flow was documented in the "Targeted updates"
  section of `UPGRADE-v2.md` (retired with the v2 tree on 2026-10-07, see
  git history).
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

- **A bench that cannot prove it provoked something measures nothing.**
  *Why* : `bare-check.sh --login` invalidated the `accessToken` to force an
  `authentication_failed`, and left the `refreshToken` valid next to it, with
  `refreshTokenExpiresAt` in the future. Claude Code took the 401, went down
  the refresh path, re-authenticated — the session stayed logged in and the
  bench asked its questions anyway. A whole bench session was lost before the
  user said « I can't trigger the unauth ». The human guard did exist (« does
  the screen appear ? ») and did not protect : one answers yes to a question
  one has no means of checking.
  *How to apply* : any bench that provokes a state must **assert by machine
  that it reached it** before asking a human anything — here a real request
  (`claude -p`), because `claude auth status` reports `loggedIn: true` off the
  file's expiry dates, never off token validity. And when invalidating a
  credential, invalidate **every recovery path**, not the first one.

- **Anchor on what survives, not on what sits next to it.** *Why* : the five
  re-anchorings of 2.1.268 all have the same shape. `model-mode-affinity` was
  anchored on the head of a comma chain upstream rewrote, while the tail it
  injects into has been byte-identical since 2.1.220. `model-selection-fix`
  re-spelled `loadUserSettings`'s preamble only to extract the `fs`/`path`
  idents from it — 268 split the method and the patch died over a body it did
  not even touch. `user-action-observer` spelled out `JSON.stringify` when it
  does not care who serialises.
  *How to apply* : before writing a regex, ask « what do I actually need ? ».
  Capture rather than spell out (the serialiser, the module, the class name) ;
  tolerate trailing arguments rather than count parameters ; prefer a site
  that is **unique in the whole bundle** (`setPreferredLocation("panel")` :
  exactly one occurrence on 220, 258 and 268) over a more readable neighbour ;
  and inject a prologue rather than re-spell upstream's code to re-emit it.

- **A sub-fix upstream has caught up with is retired on what the bundle says,
  not on a version number.** *Why* : on 2.1.268 step 4 of
  `icon-fix-open-in-current-panel` became useless — `d1$() =
  Si$()?.viewColumn ?? ViewColumn.Active` does what our injection did, and
  better (falls back to the first eligible tab group). An
  `if version >= 2.1.268` guard would have created two code paths to maintain.
  *How to apply* : test the **shape read** (« is the third argument still
  `ViewColumn.Active` ? ») and retire with a message that says so. One code
  path, and it stays correct when upstream changes its mind.

- **An artefact that names its target version must be refused, or at minimum
  flagged, when applied elsewhere.** *Why* : the patchers repo's
  `cc<version>-r<n>` tag schema DECLARES the extension it was cut for, and
  `ext-patches-sync` read that name only as a cache key, a status line and a
  fetch URL — never as an assertion. A pin left behind after four CC bumps
  (`cc2.1.258-r2`) was therefore applied as is to a 2.1.272 extension : eight
  patchers failed loudly and **nine succeeded**. The nine are the problem — a
  2.1.258 anchor that still matches in 2.1.272 code is not an anchor that
  still means the same thing, and `node --check` only proves the result
  parses. The one existing safeguard, `warn_untested_version`, compared the
  wrong reference (the `versions.json` that **travels with the downloaded
  set**, hence period-correct) and drowned under eight red lines.
  *How to apply* : when an artefact name encodes its target — tag, folder,
  digest — read that target and compare it to reality, instead of treating the
  name as opaque. Emit the alert **last**, after what it qualifies, and repeat
  it on the short-circuit path : « already applied » is precisely the sentence
  that must never stand alone when the applied set was for another version.

- **An assertion must never grep a string the fix itself quotes.** *Why* :
  `init-firewall.sh`'s capability guard explains that iptables lies by saying
  « you must be root ». The assertion meant to prove that opaque diagnostic no
  longer appears grepped… `you must be root`, and so matched the text of the
  refusal. It failed while the guard worked perfectly. Fixed to
  `Could not fetch rule set generation id`, iptables's clean signature, which
  nothing else emits.
  *How to apply* : anchor a non-regression assertion on a string **only the
  defect** can produce. If the fix message quotes the symptom — which is often
  the right wording — the quoted string is disqualified as a probe.

- **A suite that reads an environment variable must neutralise it on entry.**
  *Why* : `ext-patches-sync` prefers the environment over `.env` (compose
  injects at creation), so `toolkit.test.sh` inherited the pin, the repo and
  the token of the container running it : 69/0 on the host, **5 failures** in
  the dogfood devcontainer, same code, same commit. The suite answered about
  the container instead of answering about its fixture.
  *How to apply* : `unset` at the top of the suite every variable the code
  under test reads from the environment, and set it explicitly in the only
  tests that exercise it — the precedent is `run-firewall-suites.sh:26-29`
  with `FIREWALL_ALLOW_LOCAL_AT_REBUILD`.

- **`npm pack` silently drops any file named `.gitignore`.** *Why* : `devc
  init`'s scaffold carries a `.devcontainer/.gitignore` in
  `packages/devcontainer-cli/templates/` ; the checkout had it, every test
  passed, the tarball did not contain it — 24 files out of 25. Since the plan
  is built by walking the folder, an install from the registry would have
  scaffolded a project **without a `.gitignore`**, with no error. Found by the
  verification pass, never by the dev checkout.
  *How to apply* : name the template `_gitignore` and rename it on write
  (create-vite convention) ; and keep a test comparing `npm pack
  --dry-run --json` against the folder walk — it is the only place the defect
  is visible.

- **`npx <package> <cmd>` with no version spec does not run the local copy.**
  *Why* : in `libnpmexec` (npm 8-11), a spec with no version is a *tag* ; the
  branch calls `getManifest` (`preferOnline`) on every run, compares the
  resolved tarball URL to the local `node_modules` one, and **installs then
  runs the newer version** as soon as one exists in the registry. A
  `devDependencies` + lockfile therefore protects nothing in this form. A
  **range** spec (`@0.x`) takes the other branch : local tree queried,
  `semver.satisfies`, local copy run with no network — proven by a test
  against a dead registry (`test/npx-resolution.test.ts`).
  *How to apply* : every repeated command that goes through `npx` (an
  `initializeCommand`, a hook) carries a range or an exact version, never the
  bare name ; and avoid `^` (cmd.exe escaping) and `>` (redirection) in the
  string — `0.x` has no metacharacter.

- **Never pass an absolute path as `npx`'s first positional argument.**
  *Why* : before resolving a package, libnpmexec tests whether the requested
  command already exists as a local bin, with
  `resolve(<dir>/node_modules/.bin, args[0])`
  (`libnpmexec/lib/file-exists.js`). When `args[0]` is absolute, `resolve`
  discards the directory and returns the path itself — which exists. npx
  concludes the command is installed, **installs nothing**, never reaches
  `getBinFromManifest` (which would have translated the spec into a bin name)
  and hands the file to `sh` : `Permission denied` on a `.tgz`, without a word
  about the missing install. *How to apply* : to run a local tarball,
  `npx --yes --package /path/x.tgz -- <bin> <args>`. A **relative** path also
  works by accident (`resolve` does not short-circuit), but the `--package`
  form is the only one that says what it does.

- **A nested npm inherits the parent npm's `npm_config_*`.** *Why* :
  `npm publish --dry-run` exports `npm_config_dry_run=true` ; the `npm pack`
  run by a test under `prepublishOnly` inherits it, **exits 0 and writes no
  tarball**. The test then breaks on its own fixture
  (`assert.ok(tarball !== undefined)`) and the failure accuses the test, not
  the cause. Holds for `dry_run`, `registry`, `cache`, `production`… *How to
  apply* : every `spawn('npm'|'npx', …)` in a test starts with an environment
  cleaned of the `npm_config_*` that would change its meaning — and the
  comment says which one, otherwise the next person puts it back.

- **A shell command written and verified in bash is not verified for a reader
  in zsh.** *Why* : zsh does not split an unquoted variable expansion into
  words. `DEVC="node /x/devc.mjs"` then `$DEVC init` works in bash and fails
  in zsh on `no such file or directory: node /x/devc.mjs` — the command name
  is the whole string. A guide written from this container (bash) and run on
  the Mac (zsh) hits it on the first line, and the script carrying the same
  form internally does work — which misleads the diagnosis. *How to apply* :
  in a doc meant to be pasted into a terminal, a launcher is a **function**
  (`devc() { node "$X" "$@"; }`), never a variable. Workaround without
  rewriting : `${=VAR}`.

- **`return <promise>` inside a `try/finally` runs the `finally` before the
  promise settles** — and if that `finally` closes a resource, the promise
  never settles. *Why* : measured on `devc init`, where the « project already
  scaffolded » path did `return reportExisting(wizard)` without `await` inside
  a `try { … } finally { readline?.close() }`. The readline interface was
  created during the call then closed right away, question pending :
  `rl.question()` never settles, the event loop drains and node exits **13**
  with `Warning: Detected unsettled top-level await`. On screen, the prompt
  shows and the shell takes back control without reading anything. *How to
  apply* : in a `try` that has a `finally` releasing anything, write
  `return await f()`, never `return f()` — the rule holds even when the
  `finally` « only » logs.

- **A testable injection seam can make a lifecycle defect invisible to the
  whole suite.** *Why* : the 129 `devc init` tests all passed `options.ask`,
  so `readlineAsk()` was **never** constructed and the `finally`'s `close()`
  was a no-op — the defect above could not appear. The seam that makes prompts
  testable is exactly the one that hides the resource they hold. *How to
  apply* : when a dependency is injectable, keep **one** test that takes the
  real path (here : a `PassThrough` with `isTTY = true` and no `ask`, under
  `{ timeout }` so the failure is a readable expiry and not a hang). Checking
  that the test is worth anything is done by breaking the fix in the
  **compiled** output and verifying it goes red.

- **`node_modules/` is shared between the Mac and the container by the
  bind-mount, and a package with per-platform native binaries cannot satisfy
  both.** *Why* : `typescript@^7.0.2` ships its compiler as per-platform
  optionalDependencies (`@typescript/typescript-darwin-arm64`,
  `…-linux-arm64`, …). The Mac's `npm install` lays down only the darwin
  brick ; inside the container, `npm run build` dies on `Unable to resolve
  @typescript/typescript-linux-arm64`. Worse : `npm install` of the linux
  brick **removes** the darwin one (« added 1 package, and removed 1
  package »), so repairing one side breaks the other, and npm refuses the
  darwin one on linux (`notsup Actual cpu`). *How to apply* : to make both
  coexist, fetch the tarball and extract it by hand —
  `npm pack @typescript/typescript-<os>-<arch>@<version>` then
  `tar xzf … --strip-components=1 -C node_modules/@typescript/<name>`. And
  remember it when picking a dependency : a package with per-platform binaries
  costs this gymnastics every time
  ([.claude/rules/dependencies.md](../.claude/rules/dependencies.md)).

- **Publishing to npm from CI : trusted publishing (OIDC) + staged
  publishing, and `npm publish` left unchecked.** *Why* : npm's UI says so
  itself next to the box (« Not recommended. For stronger security, leave
  unchecked to allow staged publishing only »), and the reason is concrete,
  not decorative. `npm stage publish` drops the tarball in a holding area and
  stops ; a maintainer approves under 2FA, or discards with `npm stage reject`
  and can re-stage the same version. A published version, by contrast, is
  final past 72 h. **Pushing the tag therefore stops being the point of no
  return** — the approval is, made by a human, after the fact, with the right
  to refuse. OIDC brings the rest : no secret in the repo, nothing that
  expires, and the provenance attestation attached **without** `--provenance`.
  *How to apply* : `permissions: id-token: write`, `setup-node` with
  `registry-url` (always required, even with no token), `npm stage publish`.
  On npmjs.com, *Trusted publishing* → GitHub Actions : the account **without
  the `@`** (`meitogi`, not `@meitogi`), the repo without the owner, and the
  **workflow's file name**, which becomes load-bearing — renaming it breaks
  publication with an `E404` that looks like a missing package, and npm
  validates **nothing** at registration time. Floors : npm ≥ 11.5.1 for OIDC,
  ≥ 11.15.0 for staging ; pin `node-version: '24'` **as a literal** and never
  `node-version-file: package.json`, which would read `engines.node` and
  install a Node whose npm predates OIDC. Verified on 2026-09-21 : staging
  **preserves** provenance, which the npm docs say nowhere.

- **The container has no GitHub credential : every push is a host gesture.**
  *Why* : `gh` is installed but not authenticated, and the Dev Containers
  credential relay answers **nothing** for `github.com` — tested with and
  without a username. The trap is that a `git ls-remote` or a `git fetch` on a
  **public** repo succeeds anyway, which gives the illusion that
  authentication works ; the first `git push` destroys it with
  `could not read Password … terminal prompts disabled`. *How to apply* :
  test it **before** building a plan that assumes a push from the container
  (`printf 'protocol=https\nhost=github.com\n\n' | git credential fill` — read
  only its presence, never its value). Since `.git` is bind-mounted,
  everything Claude commits locally is immediately pushable from the Mac :
  give it the host path, which is in `$HOST_WORKSPACE_PATH`. And remind it
  that **`git push` alone does not push tags** — it answers
  `Everything up-to-date`, which reads as success while the ref stayed local
  and no run starts. `git push origin refs/tags/<tag>`.

- **When a generated artefact does not contain what was put in it, re-run the
  generator before reasoning about the sources.** *Why* : on 2026-09-30, a
  rule added to `firewall/policy.d/api.github.com.yaml` did not show up in
  `effective/policy.compiled.yaml`. I spent an hour reading
  `firewall-docker-setup.sh`, `firewall-digest.sh`, the overlay order, the
  `blocked_header_patterns` and `domains.local.txt`, and asking the user four
  times over to collect data — while `compile-policy.py`, `python3` and the
  real `firewall/` tree were available from the start. A single call
  (`compile-policy.py --config-dir <tree>/firewall --out-policy …`) produced
  the answer **and** an explicit `WARN:`. *How to apply* : as soon as a
  compiled, baked or rendered file does not reflect its source, reproduce the
  compilation locally on the **real** files, and read stderr first. The
  generator almost always says what is wrong ; inferring it from the source
  code is slower and more wrong.

- **A « it's stuck » symptom is cut in two before being diagnosed : what has
  finished, and what has not.** *Why* : same incident. The boot looked stuck,
  and I successively accused the shim, the image pull, the `/Volumes/Data`
  volume, bootstrap DNS resolution and the firewall — five hypotheses, all
  wrong. The proof was in the log the user had already pasted :
  `=== post-start done ===` followed by the `✓ all clear` panel. Every one of
  our fragments had finished ; the wait was **after**, on the VS Code side
  (extension downloads). *How to apply* : look for the last completion marker
  in the logs first. Whatever printed « done » is out of the picture, however
  slow it appears. Only propose a cause after bounding the window where the
  time actually goes.

- **Asking the user for the same measurement more than once is an error
  signal, not perseverance.** *Why* : I demanded `firewall-blocks` and an HTTP
  code four times in a row, each time formulating one more theory. *How to
  apply* : on the second request left unanswered, stop asking. Either
  reproduce it yourself with what is at hand, or write **one** script that
  collects everything at once and returns a verdict — never a third question.

- **A debug block never decides a script's exit code, and `writer | head -n`
  under `pipefail` is an intermittent failure.** *Why* : measured on
  2026-10-05 on symptems. `init-firewall.sh` printed `✓ Firewall ready`, then
  the debug dump that follows (`iptables -L`, `ipset list … | head -30`,
  redirected to `/tmp/iptables-dump.txt 2>&1`) failed ; `set -Eeuo pipefail`
  exited the script, and the ERR trap's message went **into the file**. The
  phase log showed a green firewall and a red `rc=1` fragment, with nothing in
  between — and the fragment additionally overwrote the real code with a
  hardcoded `exit 1`. Real cost : `on-create` phase aborted, VS Code re-runs
  the whole flow, create work redone at post-start, ~27 s. On `head` :
  `seq 1 200000 | head -30` fails 200/200 (SIGPIPE 141 surfaced by
  `pipefail`), `seq 1 400 | head -30` 0/200 — the boundary is the 64 KiB pipe
  buffer, so it depends on the size of the set. *How to apply* : every purely
  diagnostic block ends with `|| true` (or `|| echo …`) **and** the script
  ends with an explicit `exit 0` ; `sed -n '1,30p'` instead of `head -30` in a
  `pipefail` script ; a lifecycle fragment re-prints `rc=$?`, never an invented
  code. Guards in `test/manifest.test.sh` § « init-firewall: the exit status is
  the firewall ».

- **A cache under `.devcontainer/tmp/` lives in the workspace : « fresh
  container » does not exist for whatever reads it.** *Why* :
  `ext-patches-sync` resolved an `auto` ref from the cache first, documenting
  that tags would be queried « on a fresh container ». Since the cache
  survives the rebuild, that case never happened : `cc2.1.280-r2` was served
  the day after `r3` was published, on a recreated container, with no network
  call. *How to apply* : before writing « at first boot » or « on a fresh
  container », check where the state being read lives — `$DEVC_CONFIG_DIR/tmp/**`
  is workspace, not container. Whatever must distinguish the two must read it
  from the phase (`--create` passed by the on-create fragment), not from the
  presence of a file.

- **Nuance on the previous lesson : « done » bounds the fragments' work, not
  the phase process's duration.** *Why* : measured on 2026-10-05. `devc-hook`
  printed `=== post-start done ===` then **did not return for 30 s** — the
  per-fragment watchdog (`_frag_watchdog`, landed in 1.7.1) ran
  `while sleep 30` in the foreground of a subshell : a non-interactive bash
  defers the signal until its foreground command ends, so `run_frag`'s `kill`
  was only honoured when the `sleep` expired, and the `wait` that follows
  waited all that time. Measurement : 30.024 s for a phase containing only a
  `true` ; 0.029 s after the fix (`sleep & ; trap TERM ; wait` — `wait` is
  interruptible, a foreground `sleep` is not). The two 20-32 s « holes »
  between `on-create`, `post-create` and `post-start` in a boot cycle were
  this — not VS Code. *How to apply* : before accusing the host or VS Code of
  dead time between phases, time the phase process itself
  (`time devc-hook <phase>` against a fake `DEVC_BASE_HOOKS`), not just read
  its lines. And in any background subshell meant to be killed : never a
  blocking foreground command, always `cmd & wait $!` with a trap.

- **"The project copy is dead" says nothing about the state of what replaces
  it.** *Why* : a French-removal pass classified `skills/{diagram,tokens,
  watch-log,prepare-stack}` as out of scope because the image ships the skill
  and `slim_tree` removes the project copy — correct, and it dropped 99
  detections from the list. The image's own copy was never opened. It was
  French too, and it is the copy that reaches every project. The classification
  was right about the file and silent about the destination. *How to apply* :
  when a mechanism (`slim_tree`, a template seed, an `install.sh` overwrite)
  means "this file is not the one that matters", the next step is to open the
  one that does and apply the same check to it. A correct exclusion is still an
  exclusion — it removes a file from the list, not a problem from the tree.

- **A zero only counts if the same probe has been seen returning non-zero.**
  *Why* : a detector that reads 0 because its pattern is wrong is
  indistinguishable from one that reads 0 because the work is done. This bit
  three sessions on a `grep` for a bucket name nothing emitted. The cheap
  discipline is to run the probe on something that must fail: the French
  detector was calibrated by returning **0** on lines 1→362 of this file and
  **216** on lines 363→675 *before* any translation, then **161** on the files
  deliberately left French *after*. *How to apply* : report a zero next to a
  non-zero from the same probe, in the same run. And watch the probe's own
  blind spots — this one reads 0 on `notify/lib/smart-text.js`, which emits
  French to the user, because its word list has no `aucune` or `entrée` ; and
  it reads 1 on `'20px sans-serif'`, because `sans` is also a French word.
