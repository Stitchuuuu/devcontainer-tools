# Log — Zsh OMZ Integration

> Append-only journal. One section per delivered session. Newest at the
> bottom. Each section follows the same shape :

```
## <Session ID> — <Title>

**Date** : YYYY-MM-DD
**Files touched** :
- path/to/file1
- path/to/file2

**What** : one-paragraph summary of the change.

**Why** : the reason / constraint that drove this scope.

**Decisions** :
- _bullet — short rationale_

**Gotchas** :
- _bullet — surprise or pitfall encountered_

**Tests** :
- _command run + expected outcome_

**Commit** : `<short hash> — <commit subject>` (or "not committed yet")
```

---

## 1 — base-skeleton

**Date** : 2026-05-29
**Files touched** :
- `templates/v2/zshrc-base` (new) + `.devcontainer/zshrc-base` (dual-copy)
- `templates/v2/zshrc.local.example` (new) + `.devcontainer/zshrc.local.example` (dual-copy)
- `templates/v2/shell-init.sh` (edit head+tail) + `.devcontainer/shell-init.sh` (dual-edit)
- `templates/v2/.gitignore` (add 2 entries) + `.devcontainer/.gitignore` (dual-edit)
- `install.sh` (add 2 `copy_verbatim` calls; single edit, root file)

**What** : Built the foundation for the OMZ rollout — created the
team-wide zsh config (`zshrc-base`), the per-dev override
documentation template (`zshrc.local.example`), wired `shell-init.sh`
to source both at the right moments (zshrc-base at top before the
banner, zshrc.local at the very end so it overrides everything), and
gitignored the per-dev override + ZSH_CUSTOM plugin tree. Updated
`install.sh` so future projects adopting this template version pick
up the two new files automatically.

**Why** : The rollout splits into three sessions because building
the runtime files first lets us validate the wiring before paying
the rebuild cost. Session 1 produces zero runtime change (OMZ isn't
installed yet) but unblocks session 2 (Dockerfile install) and
session 3 (verify-rebuild).

**Decisions** :
- **Dual-edit pattern** chosen over `install.sh .` re-run. Reason : audit
  showed install.sh's wizard re-prompts PROJECT_ID with a default
  (`workspace`) that diverges from the stored value
  (`devcontainer-tools`) and does not read `.configured-setup`, so
  re-install would corrupt `{{PROJECT_ID}}`-templated files. Full
  rationale in `/home/node/.claude/plans/zshrc-fais-moi-une-async-barto.md`
  ("Mode d'exécution").
- **Gitignore = `templates/v2/.gitignore`** (= `.devcontainer/.gitignore`),
  not `.gitignore-root`. Paths relative to `.devcontainer/`. Consistent
  with existing `LESSONS.local.md` / `firewall/domains.local.txt` pattern.
- **ZSH_CUSTOM redirected to `/workspace/.devcontainer/.zsh-custom`** so
  per-dev plugin installs (e.g. `git clone … $ZSH_CUSTOM/plugins/zsh-z`)
  persist across container rebuilds (workspace is mounted, .oh-my-zsh
  isn't).
- **Theme = `robbyrussell`** (OMZ default) as team baseline. User's
  preferred `eastwood` documented as opt-in in `zshrc.local.example`.
- **Both `zsh-autosuggestions` + `zsh-syntax-highlighting`** baked as
  team default (not just one). They produce the visible upgrade
  vs. vanilla OMZ.
- **Banner risk** : verified shell-init.sh L50-220 — banner uses
  `echo`/`printf`/Python `print` only. OMZ doesn't interfere (it only
  sets `PROMPT`, which appears AFTER shell-init.sh returns). No
  changes needed to banner code.
- **Interactive gating** : both head and tail blocks gate on
  `[ -n "$ZSH_VERSION" ] && [[ $- == *i* ]]`. Skips OMZ load for
  bash and for non-interactive zsh scripts (e.g. `zsh script.sh`).

**Gotchas** :
- A `mkdir` test for `git check-ignore` left behind
  `.devcontainer/.zsh-custom/{plugins,themes}/` and a `zshrc.local`
  file. Cleaned up before sync-check. shell-init.sh will recreate
  the dir skeleton at next shell open (idempotent).
- The OMZ unattended install will fight with the `~/.zshrc` 3-line
  injection from Dockerfile.base — needs `rm -f $HOME/.zshrc
  $HOME/.zshrc.pre-oh-my-zsh` AFTER OMZ install in session 2's
  Dockerfile change. Noted there explicitly.
- Setting `ZSH_THEME` in `zshrc.local` to override the team default
  needs the user to re-source `$ZSH/oh-my-zsh.sh` (OMZ caches the
  prompt). Documented in `zshrc.local.example`.

**Tests** :
- `diff -q` between template and `.devcontainer/` copies for each of
  the 4 dual-edited files → all "identical" (no output).
- `git check-ignore -v .devcontainer/zshrc.local .devcontainer/.zsh-custom/` →
  both matched by new gitignore rules at lines 39-40 of `.devcontainer/.gitignore`.
- Full `diff -rq templates/v2/ .devcontainer/` → only pre-existing
  drift remains (firewall/, knowledge/firewall.md, .gitignore-root,
  templated files, runtime artefacts). None of the files touched by
  this session appear in the diff.
- Runtime check skipped — OMZ isn't installed yet (session 2), so
  re-sourcing `~/.zshrc` would error on `source $ZSH/oh-my-zsh.sh`.

**Commit** :
- `9fa7d25` — feat(template): ship zsh OMZ base + per-dev override skeleton (templating side, includes README port-forward section)
- `40116ec` — chore(dogfood): apply zsh OMZ skeleton + README port-forward to .devcontainer (dual-edit propagation)

The user requested the split by **target** (templating vs container) rather
than by **feature** — the README port-forward addition rode along in both
commits since it follows the same dual-edit pattern.

---

## 2 — dockerfile-omz

**Date** : 2026-05-29
**Files touched** :
- `templates/v2/Dockerfile.base` (insert RUN block)
- `.devcontainer/Dockerfile.base` (dual-edit, byte-identical block)
- `plans/zsh-omz-integration/STATUS.md` (row 2 → ✅, counter 1→2, next focus → session 3)
- `plans/zsh-omz-integration/LOG.md` (this section)
- `plans/zsh-omz-integration/EXISTING.md` ("Status after session 1" → "Status after session 2", table row for `Dockerfile.base`)

**What** : Added a single `RUN` layer to `Dockerfile.base` that
unattendedly installs Oh My Zsh under `$HOME/.oh-my-zsh`, removes the
default `.zshrc` OMZ generates, then shallow-clones two plugins
(`zsh-autosuggestions`, `zsh-syntax-highlighting`) into
`$HOME/.oh-my-zsh/custom/plugins/`. The block sits between
`USER $USERNAME` and the existing `RUN echo … >> ~/.zshrc` block, so
`rm -f $HOME/.zshrc` runs BEFORE the next layer injects the
`source shell-init.sh` line.

**Why** : Session 1 wired `shell-init.sh` to source `zshrc-base` which
in turn sources `$ZSH/oh-my-zsh.sh`, but OMZ itself was missing — every
interactive zsh prints a "no such file or directory" on that source
line (non-fatal but ugly). Baking OMZ + the two visible-upgrade plugins
into the image makes first-boot instant after the session 3 rebuild
and lets per-dev plugins layer cleanly into `$ZSH_CUSTOM`
(`/workspace/.devcontainer/.zsh-custom/`, which is workspace-mounted
and survives rebuilds).

**Decisions** :
- **Insertion point AFTER `USER $USERNAME`** — `.oh-my-zsh/` must be
  owned by `node`, not `root`. Putting it before would force a recursive
  `chown` later.
- **OMZ block BEFORE the shell-init `RUN echo`** — OMZ's `install.sh`
  always writes a default `.zshrc` (regardless of `--unattended`).
  Removing it first means the next layer starts from an empty file
  and the single-line `source shell-init.sh` injection stays clean.
- **`--depth=1` for both plugin clones** — neither plugin's history is
  consulted at runtime; the two repos drop from ~5-10 MB combined to
  ~1 MB, shaving a noticeable chunk off the image layer size.
- **`""` empty positional arg before `--unattended`** — required by
  OMZ's installer signature (`sh tools/install.sh <CUSTOM_PATH> [flags]`);
  without it, `--unattended` is interpreted as the custom path and the
  installer silently runs interactively, breaking the build.
- **Plugins NOT enabled in `plugins=()`** — `zshrc-base` already lists
  `git zsh-autosuggestions zsh-syntax-highlighting`. Installing the
  clones at the path OMZ expects (`$ZSH/custom/plugins/<name>`) is
  enough to wire them in.
- **No `RUNZSH=no` / `CHSH=no` envs needed** — `--unattended` covers
  both (verified per the open question §3 in EXISTING.md). The
  `ENV SHELL=/bin/zsh` already in the image stays authoritative.

**Gotchas** :
- The OMZ installer's behavior with `--unattended` regarding `.zshrc`
  was the deciding factor on block ordering — confirmed it always
  writes the file, hence the explicit `rm -f` before the next layer's
  injection (vs trying to suppress the write upstream).
- Firewall does NOT apply at `docker build` time — `init-firewall.sh`
  runs at container start (post-build), so the build is free to fetch
  `raw.githubusercontent.com` + `github.com/zsh-users/*`. No
  `domains.txt` change needed.
- Pre-existing drift in `diff -rq templates/v2/ .devcontainer/` is
  noisy (firewall/, hours.local logs, runtime artefacts, etc.) but
  `Dockerfile.base` does NOT appear in it post-edit. Verified.

**Tests** :
- `diff -q templates/v2/Dockerfile.base .devcontainer/Dockerfile.base`
  → no output (identical) both BEFORE and AFTER the edits.
- `diff -rq templates/v2/ .devcontainer/` → only pre-existing drift
  (firewall/, knowledge/firewall.md, .gitignore-root, templated files,
  runtime artefacts). `Dockerfile.base` absent from the list.
- `grep -n 'oh-my-zsh' templates/v2/Dockerfile.base` vs same on
  `.devcontainer/Dockerfile.base` → identical line numbers (275, 288,
  290, 292).
- Runtime check (rebuild + checklist) deferred to session 3 by design.

**Commit** :
- `bb9c7fa` — feat(template): install Oh My Zsh + autosuggest + syntax-highlight in base image (templating side : `templates/v2/Dockerfile.base` + plan files + session 2 spec)
- `0363603` — chore(dogfood): apply OMZ install to .devcontainer/Dockerfile.base (dual-edit propagation)

Split by **target** (templating vs container) per the session 1
precedent, with this session's plan-file updates riding along with
commit 1.
