# Status — Actionable sessions

> Click `→ prompt` to open the `sessions/session-NN.md` file to paste into
> a fresh Claude Code session.
> For the detailed history (reasons, files touched, gotchas), see
> [LOG.md](LOG.md). For current code state + threat surface, see
> [EXISTING.md](EXISTING.md).

## Essential sessions (in scope of rollout)

| Session | Brief | Status | Prompt |
|---|---|---|---|
| 1 | bake-firewall-config — bake whole `firewall/` dir (rules, addons, dnsmasq, **mode**, **direct-tcp-allow.txt**) + drop bind mount. Sudoers init-firewall.sh kept (Option A — session 2 hardens script). | ✅ | — |
| 2 | drop-env-injection — remove `source /tmp/.firewall-env` + helper plumbing (obsolete after session 1) | 📋 | [→ prompt](sessions/session-2-firewall-env-no-source.md) |
| 6 | adversarial-validation — replay all vectors + hunt new ones (red-team engagement) | 📋 | [→ prompt](sessions/session-6-adversarial-validation.md) |

## Optional defense-in-depth (deferred, NOT in critical path)

Ces sessions ne violent pas les 3 critères du threat model. Gardées
comme **référence** pour un futur rollout v2 si on veut renforcer la
posture (anti-persistance, anti-token-leak).

| Session | Brief | Status | Référence |
|---|---|---|---|
| 3 | claude-hooks-allowlist — validate hooks against signed allowlist | ⏸ optional | [→ spec](sessions/session-3-claude-hooks-allowlist.md) |
| 4 | firewall-mode-baked-only — MERGED INTO SESSION 1 | ✅ merged | [→ spec (historical)](sessions/session-4-bind-mount-script-validation.md) |
| 5 | mitm-log-restrict — drop node from adm + claude-logs helper | ⏸ optional | [→ spec](sessions/session-5-mitm-log-restrict.md) |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⏸ optional/deferred · ⚠️ blocked · ❌ cancelled

## Progress

- **Delivered** : 1 / 3 essential (sessions 4 mergées dans 1)
- **Next focus** : session 2 (drop-env-injection)

## Gate

Session 6 (adversarial-validation) **gate** la fin du rollout. Si elle
trouve ≥1 vecteur SUCCESS (violation d'un des 3 critères), un rollout
`devcontainer-security-hardening-v2` est ouvert.

## Threat model — les 3 critères

`node` user est **sandboxé** quand :
1. Il ne peut PAS relancer la machine seul
2. Il ne peut PAS modifier le firewall sans rebuild
3. Il ne peut PAS accéder à une ressource externe / exfiltrer sans rebuild

Tout vecteur qui viole l'un des 3 = critique, session de remédiation
dans le rollout essentiel. Tout vecteur qui ne viole AUCUN = optionnel
defense-in-depth.
