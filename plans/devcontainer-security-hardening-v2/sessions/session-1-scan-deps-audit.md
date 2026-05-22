# Session 1 — scan-deps-audit

> **Effort** : ~30 min | **Dependencies** : none (first session)

## Goal

Before tightening dnsmasq (session 3), identify ALL external domains
that `npm install` (and other package-management workflows) might
reach in this project. Pre-allowlist them in `domains.txt` so the
first strict-mode rebuild doesn't break package installation.

Use the `/scan-deps` skill to audit `package.json` for suspicious
dependencies + postinstall hooks + the domains they reach.

## Prompt to paste

`````
Je démarre la session 1 du rollout `devcontainer-security-hardening-v2`.

Entry point : `/workspace/plans/devcontainer-security-hardening-v2/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are)
- `LOG.md` (what's been done so far — empty on session 1)
- `EXISTING.md` (current code inventory + DNS architecture today)
- `sessions/session-1-scan-deps-audit.md` (this spec)

Goal : auditer les dépendances du projet via le skill `/scan-deps`
pour identifier les domaines externes accédés à `npm install` (et
particulièrement les postinstall hooks qui peuvent télécharger depuis
des CDN ou origines inattendues). Output : une liste de domaines à
**pré-allowlister** dans `domains.txt` avant le serrage dnsmasq de la
session 3.

Pourquoi maintenant : si on serre dnsmasq sans cette pré-allowlist,
le premier rebuild en mode strict casse `npm install` et impose des
itérations REFUSED-then-allowlist au runtime. Mieux vaut anticiper.

Étapes :

1. Invoquer `/scan-deps` sur le projet (ou son équivalent — checker
   skill description). Sortie attendue : analyse des dependencies,
   flag des postinstall hooks suspects, candidates POST/CDN.
2. Croiser la liste produite avec `domains.txt` baseline + wildcards
   existants (`*.statsig.com`, `*.githubusercontent.com`, etc.).
3. Pour chaque domaine identifié NON déjà couvert :
   - Vérifier sa légitimité (qui le possède, à quoi il sert)
   - Décider : ajouter à `domains.txt` (avec méthode/path appropriés)
     OU laisser (s'il sera REFUSED par v2, est-ce un break critique ?)
4. Documenter les ajouts proposés dans une section "Pré-allowlist v2"
   du LOG.md (pas encore appliqués — session 3 fera l'application).
5. Si scan-deps révèle des dépendances vraiment suspectes (ex:
   postinstall qui tente d'exfil), flag pour décision user avant tout
   ajout.

Out of scope :
- Modifier `domains.txt` (session 3 le fera)
- Modifier `dnsmasq.conf` (session 3 le fera)
- Le `compile-policy.py` (pas touché en v2)

Note : `/scan-deps` est un skill disponible, lance-le via Skill tool.

DoD at the end of this session :
1. STATUS.md : flip session 1 row 📋 → ✅, prompt link → —, bump
   Delivered counter (0→1), refresh "Next focus" to session 2.
2. LOG.md : append `## 1 — scan-deps-audit` section dated today
   avec files touched (sans doute aucun, juste une lecture) + What /
   Why / Decisions / Gotchas / Tests / Commit. Inclure la liste
   "Pré-allowlist v2" comme sous-section.
3. EXISTING.md : update si nouvelle compréhension de l'arbre
   dépendances (probablement pas nécessaire).
4. Créer `sessions/session-2-cdn-cname-enumeration.md` avec le spec
   suivant : parser `/var/log/mitmproxy.log` pour extraire les hosts
   contactés en pratique, croiser avec `domains.txt`, catégoriser le
   delta (CDN CNAME target légitime / dynamic subdomain / suspect).
5. Propose a commit (do NOT commit without explicit user confirmation).
`````

## Next session

Session 2 (cdn-cname-enumeration) — to be created at the end of
session 1 following the same minimal shape as this file. Pre-spec
already in the DoD step 4.
