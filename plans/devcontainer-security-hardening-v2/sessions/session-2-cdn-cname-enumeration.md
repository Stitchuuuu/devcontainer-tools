# Session 2 — cdn-cname-enumeration

> **Effort** : ~45 min | **Dependencies** : session 1 (delivered)

## Goal

Parser les logs mitmproxy pour extraire la liste **empirique** des
hosts contactés par Claude Code et l'environnement de dev pendant une
session typique. Croiser cette liste avec `templates/v2/firewall/domains.txt`
(en tenant compte des wildcards `*.statsig.com`, `*.githubusercontent.com`,
`*.vsassets.io`, `*.vo.msecnd.net`, `*.gallerycdn.vsassets.io`,
`*.sentry.io`) pour identifier le delta — c'est-à-dire les hosts qui
seraient **REFUSED** par le dnsmasq strict de la session 3.

Pour chaque host du delta, catégoriser :
- **CDN CNAME target légitime** → un domaine qui apparaît parce qu'un
  host allowlisté CNAME-resolve vers lui (ex: `marketplace.visualstudio.com`
  → `*.cloudfront.net`). À pré-allowlister via `domains.txt` ou un
  wildcard ciblé.
- **Dynamic subdomain d'un host allowlisté** → un sous-domaine non couvert
  par le wildcard existant (ex: `api-staging.anthropic.com` si seulement
  `api.anthropic.com` est listé). Décision case-par-case.
- **Suspect** → host inattendu, non-Anthropic, non-Microsoft, non-CDN
  connu. Flag pour décision user avant tout ajout.

L'output est une liste de candidats à pré-allowlister, documentée dans
LOG.md — **pas** de modif de `domains.txt` (session 3 le fera).

## Prompt to paste

`````
Je démarre la session 2 du rollout `devcontainer-security-hardening-v2`.

Entry point : `/workspace/plans/devcontainer-security-hardening-v2/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are — session 1 ✅, session 2 next)
- `LOG.md` (session 1 trace : 0 manifests, pré-allowlist v2 vide pour ce repo)
- `EXISTING.md` (DNS architecture today, gap #9)
- `sessions/session-2-cdn-cname-enumeration.md` (this spec)

Goal : énumérer empiriquement les hosts contactés par Claude Code +
l'environnement de dev pendant une session typique, en parsant les
logs mitmproxy. Croiser avec `templates/v2/firewall/domains.txt` (et
ses wildcards) pour identifier le delta — i.e. les hosts qui seraient
REFUSED par le dnsmasq strict de la session 3.

Pourquoi maintenant : la session 1 a couvert les manifests
applicatifs (résultat : aucun dans ce repo). Mais Claude Code et VS
Code eux-mêmes contactent des hosts non-manifestés (CDN CNAME
targets, dynamic subdomains pour A/B testing, OCSP responders, etc.).
Sans cet audit, le serrage dnsmasq de la session 3 va casser des
chemins légitimes au premier rebuild.

Étapes :

1. Identifier les fichiers de logs mitmproxy disponibles dans le
   container :
   - `/var/log/mitmproxy.log` — CONNECT events + errors
   - `/var/log/mitmproxy-writes.log` — POST/PUT/PATCH/DELETE audit
   - `/var/log/mitmproxy-passive.log` — passive log (toutes méthodes)
   - `/var/log/mitmproxy-blocks.log` — policy_enforce blocks
   Choisir le(s) plus exhaustif(s) pour l'énumération des hosts.

2. Extraire la liste unique des hosts contactés (jq + sort -u sur le
   champ `.host` des JSON lines, ou regex sur les CONNECT events
   selon le format réel des logs).

3. Croiser avec `templates/v2/firewall/domains.txt` :
   - matcher les hosts littéraux (`api.anthropic.com`, `github.com`,
     etc.)
   - matcher les wildcards (`*.statsig.com` couvre
     `prodregistryv2.org.statsig.com`)
   - identifier le **delta** = hosts contactés MAIS non couverts.

4. Catégoriser chaque host du delta :
   - **CDN CNAME target légitime** (cloudfront, fastly, akamai,
     azureedge, etc. atteint par CNAME d'un host allowlisté)
   - **Dynamic subdomain** d'un parent connu (à wildcard-iser ou
     lister explicitement)
   - **Suspect** (à flag user)

5. Pour chaque catégorie, proposer un traitement :
   - CDN target → ajouter le host littéral (pas le wildcard parent,
     cf. ROLLOUT decision « no wildcard parent promotion »)
   - Dynamic subdomain → ajouter le sous-domaine spécifique
   - Suspect → flag user, ne rien faire avant décision

6. Documenter dans LOG.md (section `## 2 — cdn-cname-enumeration`) :
   - la liste delta + catégorie + traitement proposé
   - les commandes exactes utilisées (jq, regex, etc.) pour
     reproductibilité
   - les hosts suspects flag user, si présents

Out of scope :
- Modifier `domains.txt` ou `domains.d/*.txt` (session 3 le fera)
- Modifier `dnsmasq.conf` (session 3 le fera)
- Le `compile-policy.py` (pas touché en v2)
- Modifier les addons mitmproxy

Note : si les logs sont insuffisamment populés (container fraîchement
démarré, peu de trafic), proposer à l'user de générer du trafic en
amont (`curl`, requêtes API typiques) ET de relancer la session, OU
travailler à partir d'une session précédente avec logs riches.

DoD at the end of this session :
1. STATUS.md : flip session 2 row 📋 → ✅, prompt link → —, bump
   Delivered counter (1→2), refresh "Next focus" to session 3.
2. LOG.md : append `## 2 — cdn-cname-enumeration` section dated today
   avec files touched + What / Why / Decisions / Gotchas / Tests /
   Commit. Inclure la sous-section "Delta hosts + catégorisation".
3. EXISTING.md : pas d'update probablement (juste lecture).
4. Créer `sessions/session-3-dnsmasq-strict.md` avec le spec suivant :
   éditer `templates/v2/firewall/dnsmasq.conf` (drop
   `server=127.0.0.11`), éditer `templates/v2/init-firewall.sh`
   (généraliser le claude-bridge block à un loop sur
   `direct-tcp-allow.txt`), créer
   `templates/v2/tests/integration/test-dns-strict.sh`, appliquer la
   pré-allowlist v2 cumulée (sessions 1+2) à `domains.txt`.
5. Propose a commit (do NOT commit without explicit user confirmation).
`````

## Next session

Session 3 (dnsmasq-strict) — to be created at the end of session 2
following the same minimal shape as this file. Pre-spec already
embedded in the DoD step 4.
