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

**Commit** : not committed yet (awaiting user confirmation)

**Out-of-scope addition during this session** : the user asked mid-session
to add a "Port forwarding (dev servers)" section at the top of the README
(`templates/v2/README.md` + `.devcontainer/README.md`, dual-edit) covering
VS Code's `portsAttributes` + a `wtf client dev` `.wtfcmd.yaml` example.
This addition is unrelated to the OMZ rollout and will be committed
separately (different commit scope per CLAUDE.md §10).
