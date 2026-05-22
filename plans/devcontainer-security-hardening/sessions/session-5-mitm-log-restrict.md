# Session 5 — mitm-log-restrict (OPTIONAL — defense-in-depth)

> **⚠ STATUS** : **HORS du rollout essentiel**. Defense-in-depth optionnel.
> Justification : lire les logs mitmproxy n'est pas un exfil au sens des
> 3 critères du threat model. Les tokens lus (`Authorization: Bearer ...`)
> sont déjà utilisables par `node` via l'API Anthropic légitime
> (allowlisted) → pas d'élévation, juste lecture locale. Out of scope
> per les critères "(3) pas d'exfil sans rebuild" (la lecture n'est pas
> de l'exfil).
>
> Garder ce fichier comme **référence**. À considérer en v2 si on veut
> renforcer la posture (token leakage to disk = ouvre attaque sur
> commits/PRs).

> **Effort si on le fait quand même** : ~1 h
> **Vecteur couvert** : #11 (logs mitmproxy lisibles par node — hors scope critères)
> **Workflow préservé** : `claude-logs` (nouveau helper sudo'able) remplace `cat`
> **Architecture** : 100% Dockerfile.base — pas de changement project

## TL;DR — pour les pressés

Les logs de mitmproxy contiennent **toutes les requêtes HTTP** sortantes
du container, y compris :
- Headers `Authorization: Bearer <token>` envoyés à l'API Anthropic
- Path queries (qui peuvent contenir des secrets en query string)
- Métadonnées sur ce que Claude communique

Aujourd'hui, l'utilisateur `node` peut **lire ces logs en clair** (il est
dans le groupe `adm` Linux). Si un attaquant compromet `node`, il
récupère gratuitement le token Anthropic + une vue sur les
communications passées.

Cette session **retire `node` du groupe `adm`** et fournit un helper
sudo allowlisté pour le debug légitime.

---

## Le problème — version humaine

### Métaphore

Imaginez un coffre-fort dans un bureau. À côté du coffre, un **journal de
bord** où sont notés tous les accès au coffre, **avec le code utilisé à
chaque ouverture**. Pour aider à diagnostiquer un problème (« qui a accédé
hier soir ? »), on laisse le journal accessible aux employés. Sauf que
n'importe quel employé peut maintenant **lire les codes** et utiliser un
ancien pour rentrer dans le coffre.

C'est ce qu'il se passe avec les logs mitmproxy :
- Le « coffre » = l'API Anthropic.
- Le « code d'accès » = le token OAuth dans le header `Authorization`.
- Le « journal » = `/var/log/mitmproxy.log` + `/var/log/mitmproxy-writes.log`.
- L'« employé » = l'utilisateur `node`.

### Preuve technique

Dans `Dockerfile.base` ligne 67 :

```bash
usermod -aG adm node
```

`node` est ajouté au groupe `adm`. Les logs mitmproxy sont créés avec :

```
-rw-r----- 1 mitmproxy adm /var/log/mitmproxy.log
-rw-r----- 1 mitmproxy adm /var/log/mitmproxy-writes.log
```

Mode `640` = `rw` pour le owner (mitmproxy), `r` pour le groupe (`adm`).
Comme `node` est dans `adm`, il peut lire les deux fichiers.

### Le contenu lisible

```bash
$ tail /var/log/mitmproxy-writes.log
{"ts": 1746950400.123,
 "method": "POST",
 "host": "api.anthropic.com",
 "path": "/v1/messages",
 "size": 4523,
 "ct": "application/json",
 "ua": "anthropic-cli/2.1.119"}
```

Ce log-là est métadonné uniquement (size, pas body). Mais
`/var/log/mitmproxy.log` peut contenir des fragments de requêtes
(headers en mode debug). Et le mode debug peut être activé via
`CLAUDE_CODE_FIREWALL_DEBUG=true` — variable que `node` peut influencer
en passant par `firewall-env-write` (session 2).

Plus important : si on **active mitmweb** (mode introspection) avec
mitmproxy, ou si on inspecte la mémoire du processus mitmproxy via
`/proc/<pid>/mem`... `node` ne peut pas faire ça aujourd'hui (le
process tourne sous UID `mitmproxy`), MAIS si une autre vulnérabilité
amène un privesc partiel (groupe `adm` peut éventuellement chainer avec
d'autres systemd readers), c'est une étape pivot intéressante.

### Pourquoi `node` est dans `adm` aujourd'hui

Justification dans `Dockerfile.base` :

```
# `node` joins the `adm` group here so it can read mitmproxy's logs
# under /var/log/mitmproxy{,-writes,-blocks}.log (mode 640, owner
# mitmproxy:adm). Set before the firewall RUN block so the GID is in
# place when init-firewall.sh runs.
```

Donc raison = **debug developer experience**. Le dev veut pouvoir faire
`tail /var/log/mitmproxy-writes.log` pour voir ce que Claude envoie sans
sudo prompt.

### Pourquoi c'est grave

- **Tokens lisibles** : les Authorization Bearer dans le path de l'API
  ou les query strings peuvent fuiter dans les logs niveau DEBUG.
- **Métadonnées exfiltrables** : timing, taille, fréquence des requêtes
  Anthropic permettent d'inférer ce que Claude fait. Pas critique mais
  pas négligeable.
- **Privilege creep** : `adm` est un groupe Linux "spécial" qui peut
  élargir de façon inattendue avec d'autres outils (rsyslog, journalctl,
  /var/log/auth.log selon config).

### Quel est le réel besoin debug ?

Le dev veut typiquement :
1. **Voir ce que Claude vient d'envoyer** (`tail -f mitmproxy-writes.log`)
2. **Compter les requêtes par host** (`awk` sur les logs)
3. **Trouver pourquoi un host est bloqué** (`grep host /var/log/mitmproxy.log`)

Toutes ces opérations peuvent se faire via **un helper sudo allowlisté**
qui lit + affiche les logs, sans donner accès direct à `node`.

---

## La solution

### Principe

1. **Retirer `node` du groupe `adm`** dans `Dockerfile.base`.
2. **Créer un helper** `/usr/local/bin/claude-logs` qui lit les logs
   mitmproxy et les affiche. Helper sudo'able sans prompt (`NOPASSWD`)
   mais **lecture seule, valeur restreinte**.
3. **Documenter** : pour debug logs, utiliser `claude-logs` au lieu de
   `tail` direct.

### Le helper `claude-logs`

```bash
#!/bin/bash
# claude-logs — read-only access to mitmproxy logs for diagnostic purposes.
# Designed to be invoked via sudo (NOPASSWD allowlisted for user `node`).
# Strictly read-only : never writes, never accepts shell metachars in args.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage : claude-logs [--lines N] [--follow] [--writes|--connects|--blocks] [--filter PATTERN]

  --lines N         Last N lines (default 50, max 1000)
  --follow          Tail -f mode (Ctrl+C to exit)
  --writes          mitmproxy-writes.log (POST/PUT/PATCH per request)
  --connects        mitmproxy.log (CONNECT events + errors)
  --blocks          mitmproxy-blocks.log (policy_enforce blocks)
  --filter PATTERN  grep filter (fixed string, max 64 chars, [A-Za-z0-9._-/])

Examples :
  claude-logs --writes --lines 20
  claude-logs --connects --follow
  claude-logs --writes --filter api.anthropic.com
EOF
}

FILE=/var/log/mitmproxy-writes.log
LINES=50
FOLLOW=0
FILTER=""

while [ $# -gt 0 ]; do
  case "$1" in
    --writes)   FILE=/var/log/mitmproxy-writes.log; shift ;;
    --connects) FILE=/var/log/mitmproxy.log;        shift ;;
    --blocks)   FILE=/var/log/mitmproxy-blocks.log; shift ;;
    --lines)    LINES="$2"; shift 2 ;;
    --follow)   FOLLOW=1; shift ;;
    --filter)   FILTER="$2"; shift 2 ;;
    -h|--help)  usage; exit 0 ;;
    *) echo "Unknown arg: $1"; usage; exit 1 ;;
  esac
done

# Validation
if ! [[ "$LINES" =~ ^[0-9]+$ ]] || [ "$LINES" -gt 1000 ]; then
  echo "Invalid --lines (must be 0..1000)" >&2
  exit 1
fi
if [ -n "$FILTER" ] && ! [[ "$FILTER" =~ ^[A-Za-z0-9._/-]{1,64}$ ]]; then
  echo "Invalid --filter (must be [A-Za-z0-9._-/]{1,64})" >&2
  exit 1
fi
if [ ! -f "$FILE" ]; then
  echo "Log file does not exist (yet) : $FILE" >&2
  exit 0
fi

# Execute
if [ "$FOLLOW" = 1 ]; then
  CMD=(tail -n "$LINES" -F "$FILE")
else
  CMD=(tail -n "$LINES" "$FILE")
fi

if [ -n "$FILTER" ]; then
  "${CMD[@]}" | grep -F "$FILTER"
else
  "${CMD[@]}"
fi
```

Allowlisté via sudoers :

```
node ALL=(root) NOPASSWD: /usr/local/bin/claude-logs
```

### Comment l'utiliser

```bash
# Avant (cassait après cette session — node n'est plus dans adm) :
tail -f /var/log/mitmproxy-writes.log
# → permission denied

# Après :
sudo claude-logs --writes --follow
# → fonctionne, mais lecture seule, args validés
```

Le helper est **complètement lecture seule** — pas de risque d'exécution
arbitraire via injection d'argument car les args sont strictement
validés par regex.

---

## Impact sur le workflow dev

### Ce qui CHANGE

| Action | Avant | Après |
|---|---|---|
| `tail /var/log/mitmproxy-writes.log` | OK | Permission denied |
| `tail /var/log/mitmproxy.log` | OK | Permission denied |
| Voir les logs en debug | `tail` direct | `sudo claude-logs --writes` |
| Filtrer les logs | `tail | grep ...` | `sudo claude-logs --writes --filter ...` |

### Ce qui NE CHANGE PAS

- ✅ mitmproxy continue de fonctionner.
- ✅ Les logs continuent d'être écrits.
- ✅ Un user qui ne touchait jamais aux logs (95% des devs) ne voit aucune
  différence.

### Alias suggestion

Optionnel — `shell-init.sh` (baked, session 4) peut ajouter :

```bash
alias mitm-tail='sudo claude-logs --writes --follow'
alias mitm-grep='sudo claude-logs --writes --filter'
```

Pour préserver la fluidité du workflow.

---

## Implementation détaillée

### Fichier 1 — `templates/v2/firewall/claude-logs` (nouveau)

Créer le script avec le contenu ci-dessus. `chmod +x`.

> Le nom du répertoire `firewall/` est sémantiquement discutable
> (`claude-logs` n'est pas vraiment du firewall) — alternative : placer
> dans `templates/v2/host-helpers/` côté workspace, mais c'est un
> binaire container-side. Choix : créer un répertoire `templates/v2/bin/`
> pour les binaires baked container-side qui ne sont pas du firewall.
> Décision en début de session.

### Fichier 2 — `templates/v2/Dockerfile.base`

**Retirer** `node` du groupe `adm` :

```dockerfile
# Avant :
RUN useradd --system --no-create-home --shell /usr/sbin/nologin mitmproxy && \
    usermod -aG adm node && \
    [...]

# Après :
RUN useradd --system --no-create-home --shell /usr/sbin/nologin mitmproxy && \
    [...]
# (la ligne usermod -aG adm node est supprimée)
```

**Ajouter** le COPY du helper :

```dockerfile
COPY bin/claude-logs /usr/local/bin/claude-logs
RUN chmod 0755 /usr/local/bin/claude-logs && \
    chown root:root /usr/local/bin/claude-logs
```

**Étendre** la sudoers entry :

```
node ALL=(root) NOPASSWD: /usr/local/bin/init-firewall.sh,
                          /usr/local/bin/test-firewall.sh,
                          /usr/local/bin/firewall-env-write,
                          /usr/local/bin/claude-logs
```

### Fichier 3 — `templates/v2/shell-init.sh` (baked via session 4)

Ajouter les alias optionnels :

```bash
# Logs mitmproxy — sudo'able read-only helper (session 5 — security hardening)
alias mitm-tail='sudo /usr/local/bin/claude-logs --writes --follow'
alias mitm-grep='sudo /usr/local/bin/claude-logs --writes --filter'
alias mitm-connects='sudo /usr/local/bin/claude-logs --connects'
```

### Fichier 4 — `templates/v2/SECURITY.md`

Documenter le changement :

```markdown
## Logs mitmproxy

Pour des raisons de sécurité (session 5), l'utilisateur `node` n'a plus
accès direct aux logs mitmproxy. Utiliser :

  sudo claude-logs --writes [--follow] [--filter PATTERN]
  sudo claude-logs --connects [...]
  sudo claude-logs --blocks [...]

Les alias `mitm-tail`, `mitm-grep`, `mitm-connects` sont disponibles
dans le shell.
```

### Fichier 5 — `.devcontainer/` (dogfooding mirror)

Copier `Dockerfile.base`, `shell-init.sh`, `bin/claude-logs`.

---

## Verification

### 1. `node` ne lit plus les logs directement

```bash
cat /var/log/mitmproxy-writes.log
# Attendu : Permission denied

ls -la /var/log/mitmproxy*.log
# Attendu : -rw-r----- mitmproxy adm
#           node N'EST PAS dans adm

groups
# Attendu : node (sans "adm")
```

### 2. `claude-logs` fonctionne

```bash
sudo claude-logs --writes --lines 5
# Attendu : affiche les 5 dernières lignes de mitmproxy-writes.log

sudo claude-logs --writes --filter api.anthropic.com --lines 10
# Attendu : 10 lignes filtrées
```

### 3. `claude-logs` rejette les inputs invalides

```bash
# Pattern shell-meta
sudo claude-logs --filter '$(rm -rf /)'
# Attendu : "Invalid --filter (must be [A-Za-z0-9._-/]{1,64})"

# Lines hors range
sudo claude-logs --lines 99999
# Attendu : "Invalid --lines (must be 0..1000)"

# Subcommand injection
sudo claude-logs --writes --filter 'api.anthropic.com; cat /etc/shadow'
# Attendu : rejected by regex
```

### 4. Pas de chemin "creative" pour accéder

```bash
# Tentative via lecture procfs
cat /proc/$(pidof mitmdump)/fd/* 2>&1 | head -5
# Attendu : Permission denied sur tous

# Tentative via journalctl (si node est ajouté à un autre groupe lié)
journalctl 2>&1 | head -5
# Attendu : "No journal files were found." ou Permission denied
```

---

## Edge cases

### EC1 — Un script du dev existant utilise `tail /var/log/mitmproxy.log`

Trouver et remplacer dans :
- `test-firewall.sh` (si applicable)
- Documentation interne
- Scripts de debug perso

L'erreur sera bruyante (Permission denied) donc facile à identifier.

### EC2 — Le user veut activer `mitmweb` (interface graphique)

Hors scope de cette session. Si besoin, design un wrapper `claude-logs-web`
qui lance mitmweb sur un port restreint + authentification HTTP basic.

### EC3 — Logs très volumineux (rotation manquante)

Pas dans le scope sécurité, mais à noter : `/var/log/mitmproxy*.log`
n'a pas de rotation configurée. Sur un projet long, ça peut grossir.
Hors scope ici, ouvrir un ticket follow-up (logrotate).

---

## Rollback

```bash
git revert <hash>
docker rmi claude-devcontainer-base:$(grep '^CLAUDE_CODE_VERSION=' .devcontainer/.env | cut -d= -f2)
# Rebuild
```

---

## DoD

1. **STATUS.md** : flip 5 row 📋 → ✅, prompt → —, Delivered 4→5, Next focus = session 6 (adversarial).
2. **EXISTING.md** : vecteur #11 passe 🔴 → 🟢.
3. **LOG.md** : append `## 5 — mitm-log-restrict` daté.
4. **Créer** `sessions/session-6-adversarial-validation.md` (déjà
   existant — confirmer cohérence).
5. **Proposer commit** :
   `security: drop node from group adm, provide claude-logs helper for read-only mitm log access`

---

## Prompt à coller

`````
Je démarre la session 5 du rollout `devcontainer-security-hardening`.

Entry point : `/workspace/plans/devcontainer-security-hardening/ROLLOUT.md`
Read : STATUS.md, LOG.md, EXISTING.md, this file
       (sessions/session-5-mitm-log-restrict.md).

Goal : éliminer le vecteur #11 (node lit les logs mitmproxy avec bodies
API en clair). Retirer node du groupe adm. Provide claude-logs helper
sudo'able read-only avec validation stricte des arguments.

Décision en début de session : `templates/v2/bin/` (nouveau répertoire
pour les binaires baked container-side) vs `templates/v2/firewall/` (sémantiquement
moins juste). Recommandation : créer `bin/` séparé.

Suivre "Implementation détaillée" + "Verification" + "DoD". Vérifier
EC1 (scripts existants qui utilisent tail direct).
`````
