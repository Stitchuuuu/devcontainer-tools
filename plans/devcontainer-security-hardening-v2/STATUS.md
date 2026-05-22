# Status — Actionable sessions

> Click `→ prompt` to open the `sessions/session-NN.md` file to paste into
> a fresh Claude Code session.
> For the detailed history (reasons, files touched, gotchas), see
> [LOG.md](LOG.md). For current code state, see [EXISTING.md](EXISTING.md).

| Session | Brief | Status | Prompt |
|---|---|---|---|
| 1 | scan-deps-audit — `/scan-deps` over project, pre-allowlist npm install domains | ✅ | — |
| 2 | cdn-cname-enumeration — parse mitmproxy.log, list hosts contacted in practice, categorise delta vs domains.txt | ✅ | — |
| 3 | dnsmasq-strict — drop catch-all server, generalise sibling resolve, add integration test | ✅ | — |
| 4 | adversarial-validation — gate session, replay PoC #9, verify no regression | ✅ | — |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⚠️ blocked · ❌ cancelled

## Progress

- **Delivered** : 4 / 4
- **Next focus** : v2 ferme — rollout complet.

## Gate

Session 4 (adversarial-validation) **a passé la gate** le 2026-05-22 :
PoC #9 replay sur HEAD = `2cd3cd6` retourne `status: REFUSED` + payload
absent des 3 mitmproxy logs ; `test-dns-strict.sh` 6/0/1 ;
`test-firewall.sh` 0 ❌ / 2 ⚠️ pré-existants / 2 ℹ️ cloud mode ;
critères 1+2+3 du threat model v1 tous tenus. Voir [LOG.md §4](LOG.md)
pour les outputs verbatim.
