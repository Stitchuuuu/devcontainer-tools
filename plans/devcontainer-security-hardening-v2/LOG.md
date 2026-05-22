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
