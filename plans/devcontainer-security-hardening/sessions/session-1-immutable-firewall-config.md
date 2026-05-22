# Session 1 — bake-firewall-config

> **Effort** : ~3-4 h | **Vecteurs couverts** : #1, #2, #3, #8, #10 + firewall-mode + CLAUDE_CODE_FIREWALL_ALLOWED
> **Workflow préservé** : `edit .devcontainer/firewall/* → rebuild → appliqué`
> **Scope étendu** (revue 2026-05-22) : absorbe ce qui était session 4 (firewall mode bake)

## TL;DR — pour les pressés

Aujourd'hui, le firewall du devcontainer est **configurable depuis l'intérieur
du container par n'importe quel utilisateur**, alors que ça ne devrait pas
l'être. La cause est un détail technique de Docker : un fichier marqué
« lecture seule » qui en réalité ne l'est pas.

Cette session **bake TOUT ce qui est sécurité-critique dans l'image** :
- Règles firewall (`domains.txt`, `policy.d/`, `policy.local.d/`, etc.)
- Addons mitmproxy (`addons/*.py`)
- Config dnsmasq (`dnsmasq.conf`)
- **Mode firewall** (`default-mode` — strict par défaut)
- **Hosts autorisés** (`direct-tcp-allow.txt` — pour les services internes
  comme `claude-bridge:9223`)

Conséquence : pour modifier n'importe quel paramètre firewall, il faudra
**rebuild l'image** (quelques secondes). Plus aucun changement à la
volée n'est possible.

`.env` reste workspace-éditable pour les variables NON sécurité-critiques
(TZ, CLAUDE_CODE_VERSION, etc.). Seules les vars qui influencent le
firewall sont déplacées en baked.

---

## Le problème — version humaine

### Métaphore

Imaginez un coffre-fort dans un bureau. La porte du coffre est marquée
« interdiction de modifier le contenu ». Mais le mur du fond du coffre est
en carton et donne sur le couloir. N'importe qui peut **passer par le
couloir**, modifier le contenu, puis ressortir comme s'il n'avait jamais
touché le coffre.

C'est exactement ce qu'il se passe avec le firewall :
- La « porte du coffre » = `/etc/devcontainer-firewall/` (marqué `:ro`,
  lecture seule).
- Le « mur en carton » = `/workspace/.devcontainer/firewall/` (le même
  répertoire physique, mais accessible en écriture via un autre chemin).

### Preuve technique

Voici ce qu'on peut faire DÈS MAINTENANT depuis le container, en tant que
`node` (utilisateur non-privilégié) :

```bash
# Tentative directe — bloquée, comme attendu
$ echo "evil.com" > /etc/devcontainer-firewall/domains.local.txt
zsh: read-only file system

# Mais via le workspace — passe sans problème
$ echo "evil.com" > /workspace/.devcontainer/firewall/domains.local.txt
$ cat /etc/devcontainer-firewall/domains.local.txt
evil.com
```

Le fichier marqué « lecture seule » contient maintenant `evil.com` parce
que c'est le **même fichier physique** sur l'hôte.

### Pourquoi c'est grave

Si Claude est manipulé (prompt injection via un fichier ou message
hostile) ou si un attaquant prend la main sur le terminal, il peut, en
**5 secondes** :

1. Ajouter `mon-serveur-attaque.com` à la liste blanche du firewall.
2. Demander à VS Code de fermer-rouvrir le container (« Reload Window »
   — un geste qu'un dev fait 20 fois par jour).
3. Le firewall, au redémarrage, applique la nouvelle config qui inclut
   désormais le serveur de l'attaquant.
4. Envoyer tout le contenu du workspace vers ce serveur, légitimement,
   à travers le firewall.

### Quelles vulnérabilités exactement ?

Cette même cause racine permet **7 attaques différentes** :

| # | Attaque | Comment |
|---|---|---|
| 1 | Ajouter un host arbitraire à la liste blanche | Modifier `domains.local.txt` |
| 2 | Injecter du code Python dans le proxy d'inspection | Modifier `addons/*.py` |
| 3 | Détourner un nom de domaine légitime vers un attaquant | Modifier `dnsmasq.conf` |
| 8 | Élargir silencieusement les permissions sur un host déjà autorisé | Modifier `policy.d/*.yaml` |
| 10 | Modifier les patterns de blocage | Modifier `firewall-blocks` |
| (mode) | Désactiver le firewall (mode `off`) | Modifier `.configured-firewall-mode` workspace |
| (allowed) | Ajouter un host:port arbitraire à iptables ACCEPT | Modifier `CLAUDE_CODE_FIREWALL_ALLOWED` dans `.env` |

Toutes exploitables au prochain **reload VS Code**. Aucune n'apparaît
dans `git status` de façon évidente — les fichiers en question sont
prévus pour être édités par le dev.

#### Le sous-trou « mode » (découvert post-audit initial)

`firewall-mode.sh` permet à `node` de switcher le firewall en mode `off`
(kill-switch légitime documenté). Mais en mode off, **iptables est
flushé complètement → exfiltration libre**. Sans cette session, modifier
`.configured-firewall-mode` + reload = bypass.

#### Le sous-trou « allowed-hosts » (découvert post-audit initial)

`CLAUDE_CODE_FIREWALL_ALLOWED=host:port` dans `.env` ajoute host:port à
l'iptables OUTPUT en ACCEPT direct (bypass mitmproxy). `.env` est
workspace-éditable. Modifier + reload = ajouter `evil.com:443` à
l'ACCEPT direct → exfil libre.

---

## La solution

### Principe

Au lieu de **lire** la config firewall depuis le disque hôte à chaque
démarrage, on l'**embarque** dans l'image Docker au moment du build.

Concrètement :
- On supprime la ligne du `docker-compose.yml` qui dit « pour
  `/etc/devcontainer-firewall/` dans le container, va voir
  `./firewall/` sur l'hôte ».
- À la place, dans le `Dockerfile`, on dit « `COPY ./firewall/`
  vers `/etc/devcontainer-firewall/` ». Cette copie a lieu **au build**.
- **NEW** : on ajoute deux nouveaux fichiers à `firewall/` :
  - `default-mode` (contient `strict` par défaut) — lu par
    `init-firewall.sh` au lieu de la variable `FIREWALL_MODE`
  - `direct-tcp-allow.txt` (contient `claude-bridge:9223` etc.) — lu par
    `init-firewall.sh` au lieu de la variable `CLAUDE_CODE_FIREWALL_ALLOWED`
- `init-firewall.sh` est modifié pour lire ces fichiers baked AU LIEU
  des variables d'environnement.
- Conséquence : `firewall-env-write` (et tout le mécanisme env-injection
  associé) devient inutile. Sa suppression est traitée en session 2.

Après build, la config firewall (règles, mode, hosts directs) est
**gravée dans l'image**, comme un binaire système. Aucune façon de la
modifier sans rebuild.

### Ce qui reste dans `.env` (workspace-éditable)

`.env` continue de servir pour les variables NON sécurité-critiques :
- `TZ=Europe/Paris`
- `CLAUDE_CODE_VERSION=2.1.145`
- `DC_PROJECT=...`
- `CLAUDE_CONFIG_DIR=/home/node/.claude` (mode local Ollama)
- `ANTHROPIC_BASE_URL=...` (mode local)
- `CLAUDE_CODE_FIREWALL_DEBUG=true|false` (informatif, pas exfil)

`.env` est éditable par le user (workspace bind mount). Modifier `.env`
n'a aucun effet sécurité-critique. Bonne UX préservée.

### Pourquoi cette approche plutôt qu'une autre ?

| Alternative | Pourquoi rejetée |
|---|---|
| Mettre un `:ro` plus profond (mount overlay) | Le fichier hôte reste écrivable côté host ; au reload Docker re-lit la donnée modifiée. Cosmétique. |
| Chiffrer la config et déchiffrer au boot | Ajoute de la complexité ; la clé devrait vivre quelque part dans l'image, donc même résultat. |
| Signer la config et vérifier au boot | Possible mais coûteux à maintenir (clés, rotation). Bake-only atteint le même objectif sans clés. |
| Empêcher node d'écrire dans `/workspace/.devcontainer/` via permissions | Casse les workflows légitimes (commit de la config, edit côté dev). |

**Bake-only** = la solution la plus simple qui satisfait le threat model
« Claude untrusted, host trusted ».

---

## Impact sur le workflow dev

### Ce qui CHANGE

| Action | Avant | Après |
|---|---|---|
| Modifier `domains.txt`, `policy.d/`, `addons/`, `dnsmasq.conf` | Edit + `sudo init-firewall.sh` → appliqué dans la seconde | Edit + **Rebuild Container** (~5s pour la couche project) → appliqué |
| Tester une nouvelle policy | Edit + sudo init | Edit + Rebuild |
| Ajouter un domaine local dans `domains.local.txt` | Edit + sudo init | Edit + Rebuild |

### Ce qui NE CHANGE PAS

- ✅ L'édition des fichiers `.devcontainer/firewall/*` **dans le
  devcontainer** reste possible (VS Code, vi, etc.).
- ✅ Le commit git de ces fichiers fonctionne normalement.
- ✅ Le mode `off` / `basic` / `strict` via `firewall-mode.sh` reste
  fonctionnel (lit le flag `.configured-firewall-mode`, qui n'est PAS
  dans le bake).
- ✅ Le workflow normal du dev (95% du temps il ne touche pas au
  firewall) est inchangé.

### Coût concret du rebuild

Modification courante (domains.local.txt, policy.local.d/) → rebuild du
**project layer** uniquement → **5-10 secondes**. Acceptable.

Modification du baseline (`domains.txt`, `addons/`, `dnsmasq.conf`) →
rebuild du **base layer** aussi → **5-8 minutes**, mais cela arrive
**rarement** (changements de policy globale, pas du quotidien).

---

## Implementation détaillée

### Fichier 1 — `templates/v2/docker-compose.yml`

**Supprimer** la ligne :

```yaml
- ./firewall:/etc/devcontainer-firewall:ro
```

Le bloc volumes après modif :

```yaml
volumes:
  - ..:/workspace:delegated
  - ./vscode-settings.json:/workspace/.vscode/settings.json:bind
  - bash-history:/commandhistory
  - claude-config:/home/node/.claude
  - claude-creds:/home/node/.claude-creds
  - mitmproxy:/var/lib/mitmproxy
```

### Fichier 2 — `templates/v2/Dockerfile` (project layer) — RESTER MINIMAL

> **Règle architecture** : `Dockerfile` projet doit être **court** — un
> maximum de COPY génériques vit dans `Dockerfile.base`. On veut un
> project Dockerfile « clean », pas 50 lignes. Donc **un seul `COPY`**
> pour tout le répertoire firewall projet :

```dockerfile
USER root

# Tout le répertoire firewall projet en une seule COPY. install.sh garantit
# que tous les sous-éléments attendus existent (domains.local.txt vide,
# policy.local.d/ vide, domains.d/ vide, default-mode = "strict",
# direct-tcp-allow.txt vide). firewall-docker-setup.sh (baked dans
# Dockerfile.base) gère perms + chown.
COPY firewall/ /etc/devcontainer-firewall/
RUN /usr/local/bin/firewall-docker-setup.sh

USER node
```

2 lignes au lieu de 5+. Le COPY récursif overwrite les fichiers baseline
de `Dockerfile.base` quand un projet en fournit (override pattern :
acceptable et même souhaitable pour `addons/`, `dnsmasq.conf` quand un
projet veut les patcher).

**Pour `Dockerfile.php`** : pareil — un seul COPY. Pas de duplication
ligne par ligne.

### Fichier 2b — `templates/v2/firewall/default-mode` (NOUVEAU)

Contenu unique :
```
strict
```

### Fichier 2c — `templates/v2/firewall/direct-tcp-allow.txt` (NOUVEAU)

> **Sémantique précise** : ce fichier liste les `host:port` autorisés en
> **TCP direct, bypass mitmproxy**. Pas pour les flux HTTP/HTTPS (ceux-ci
> passent par mitmproxy via `HTTPS_PROXY=127.0.0.1:8080` et sont
> contrôlés par `domains.txt` + `policy.d/`, indépendamment du port).
>
> Cas d'usage typique : services internes Docker qui parlent un
> **protocole TCP custom non-HTTP** (sidecar `claude-bridge:9223`
> avec sa propre API binaire, serveur Redis interne, etc.).
>
> Pour un service HTTP/HTTPS sur un port non-standard
> (ex: `https://api.example.com:3000`) : NE PAS l'ajouter ici. Mitmproxy
> gère n'importe quel port HTTP/HTTPS — il suffit d'ajouter
> `api.example.com` à `domains.txt` normalement.

**Format** : une entrée `host:port` par ligne. Mêmes conventions que
`.env` :
- Lignes vides ignorées
- Lignes commençant par `#` = commentaires (le switch on/off d'un
  service se fait en commentant/décommentant la ligne)
- Espaces de début/fin trimées
- Inline comment supporté : `claude-bridge:9223  # comment`

**Mot-clé spécial** : `host` = résolu vers `host.docker.internal`
(Docker gateway IP). Migration du special case existant dans
init-firewall.sh.

**Exemple par défaut — RIEN d'ouvert + exemples commentés** :

Principe : par défaut, aucune entrée active. Mais le fichier contient
des **exemples commentés** des hosts classiques (style `.env.example`)
pour que le dev sache quoi décommenter s'il veut éditer manuellement.
`claude-switch` automatise normalement ce travail.

```
# direct-tcp-allow.txt — services autorisés en TCP direct (bypass mitmproxy)
#
# Format : host:port par ligne. Commentaires : # en début de ligne ou
# en inline. Mot-clé spécial : "host" = host.docker.internal.
#
# ⚠ Mettre ici UNIQUEMENT des services non-HTTP. Pour HTTP/HTTPS sur
# n'importe quel port : utiliser domains.txt (mitmproxy s'en occupe).
#
# Par défaut RIEN n'est actif (cloud mode). Pour switcher :
#   .devcontainer/host-helpers/claude-switch local-bridge   (puis rebuild)
#   .devcontainer/host-helpers/claude-switch local-direct   (puis rebuild)
#   .devcontainer/host-helpers/claude-switch cloud          (puis rebuild)
#
# Exemples (décommenter manuellement OU laisser claude-switch les gérer) :

# Sidecar Claude bridge (UniClaudeProxy) — utilisé en mode local-bridge
# claude-bridge:9223

# Ollama serveur sur l'host (port 11434) — utilisé en mode local-direct
# host:11434
```

**Interaction avec `claude-switch`** :

`claude-switch` (host-helper) toggle entre cloud / local-bridge /
local-direct. **Modifie 2 fichiers**, pas un nouveau :

| Action | `.env` | `direct-tcp-allow.txt` |
|---|---|---|
| `cloud` (défaut) | Pas de `ANTHROPIC_BASE_URL` | Vide (que commentaires) |
| `local-bridge` | `ANTHROPIC_BASE_URL=http://claude-bridge:9223` | `claude-bridge:9223` |
| `local-direct` | `ANTHROPIC_BASE_URL=http://host.docker.internal:11434` | `host:11434` |

**Conséquence UX** : `claude-switch` nécessite **toujours un rebuild**
(car `direct-tcp-allow.txt` est baked). Avant : reload suffisait.
Maintenant : rebuild systématique.

**Justification** :
- Par défaut, **aucun port ouvert** pour rien (principe minimal surface).
- 1 changement de mode = 1 action consciente = 1 rebuild. Acceptable
  pour une opération rare (typiquement quelques fois par projet).
- Cohérent avec « modifier le firewall = rebuild ».

**Implementation `claude-switch`** :

```bash
#!/usr/bin/env bash
# claude-switch.sh — switch Claude routing mode (cloud / local-bridge / local-direct)
set -euo pipefail
MODE="${1:-}"
case "$MODE" in cloud|local-bridge|local-direct) ;;
  *) echo "Usage: $0 cloud|local-bridge|local-direct" >&2; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$SCRIPT_DIR/.env"
TCP_FILE="$SCRIPT_DIR/firewall/direct-tcp-allow.txt"

# Helper : ré-écrire ANTHROPIC_BASE_URL dans .env
set_env_var() {
  local key="$1" value="$2"
  touch "$ENV_FILE"
  # Strip existing
  sed -i.bak "/^${key}=/d" "$ENV_FILE"
  rm -f "$ENV_FILE.bak"
  # Add if value non-empty
  [ -n "$value" ] && echo "${key}=${value}" >> "$ENV_FILE"
}

# Helper : écrire/effacer entrée dans direct-tcp-allow.txt
set_tcp_entry() {
  local entry="$1"  # vide pour effacer
  # Strip toutes les entrées non-commentaire existantes
  sed -i.bak -E '/^[[:space:]]*[^#[:space:]]/d' "$TCP_FILE"
  rm -f "$TCP_FILE.bak"
  [ -n "$entry" ] && echo "$entry" >> "$TCP_FILE"
}

case "$MODE" in
  cloud)
    set_env_var ANTHROPIC_BASE_URL ""
    set_tcp_entry ""
    ;;
  local-bridge)
    set_env_var ANTHROPIC_BASE_URL "http://claude-bridge:9223"
    set_tcp_entry "claude-bridge:9223"
    ;;
  local-direct)
    set_env_var ANTHROPIC_BASE_URL "http://host.docker.internal:11434"
    set_tcp_entry "host:11434"
    ;;
esac

cat <<EOF
✓ Mode '$MODE' configuré :
  - .env updated (ANTHROPIC_BASE_URL)
  - firewall/direct-tcp-allow.txt updated

→ Rebuild the container in VS Code to apply.
EOF
```

**Gap résiduel accepté** : interception locale via loopback.
- PoC : `node` listen sur 127.0.0.1:9999, modifie `.env :
  ANTHROPIC_BASE_URL=http://127.0.0.1:9999`, reload, Claude POST son
  prompt à 127.0.0.1:9999.
- Pourquoi accepté : les données restent **dans le container**. Pas
  d'exfil externe. Ne viole pas critère #3.
- Mitigation v2 : restreindre loopback ACCEPT aux ports 8080 + 53.

### Fichier 2d — `templates/v2/install.sh`

Garantir les nouveaux fichiers existent :

```bash
# Section setup_firewall_files()
touch "$TARGET/.devcontainer/firewall/domains.local.txt"
mkdir -p "$TARGET/.devcontainer/firewall/policy.local.d"
mkdir -p "$TARGET/.devcontainer/firewall/domains.d"

# NEW (session 1 extended) :
[ -f "$TARGET/.devcontainer/firewall/default-mode" ] || \
    echo "strict" > "$TARGET/.devcontainer/firewall/default-mode"
[ -f "$TARGET/.devcontainer/firewall/direct-tcp-allow.txt" ] || \
    : > "$TARGET/.devcontainer/firewall/direct-tcp-allow.txt"

# Migration legacy (one-shot) :
# - .configured-firewall-mode → firewall/default-mode
if [ -f "$TARGET/.devcontainer/.configured-firewall-mode" ] && \
   [ ! -s "$TARGET/.devcontainer/firewall/default-mode" ]; then
  cp "$TARGET/.devcontainer/.configured-firewall-mode" \
     "$TARGET/.devcontainer/firewall/default-mode"
fi
# - CLAUDE_CODE_FIREWALL_ALLOWED dans .env → firewall/direct-tcp-allow.txt
if grep -q '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$TARGET/.devcontainer/.env" 2>/dev/null; then
  grep '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$TARGET/.devcontainer/.env" \
    | cut -d= -f2- | tr ',' '\n' \
    >> "$TARGET/.devcontainer/firewall/direct-tcp-allow.txt"
  # Optionnel : retirer la ligne de .env (laisse au admin de projet)
  echo "→ migrated CLAUDE_CODE_FIREWALL_ALLOWED to firewall/direct-tcp-allow.txt"
fi
```

### Fichier 2e — `templates/v2/init-firewall.sh` — lire depuis baked

```bash
# Avant (lit l'env) :
# FIREWALL_MODE=${FIREWALL_MODE:-strict}
# IFS=',' read -ra ENTRIES <<< "$CLAUDE_CODE_FIREWALL_ALLOWED"

# Après (lit baked files) :
FIREWALL_MODE=$(cat /etc/devcontainer-firewall/default-mode 2>/dev/null || echo strict)
case "$FIREWALL_MODE" in
  strict|basic|off|paranoid|okeish) ;;
  *) FIREWALL_MODE=strict ;;
esac

# direct-tcp-allow.txt : une entrée host:port par ligne.
# Conventions .env-style :
#   - lignes vides ignorées
#   - lignes commençant par # ignorées (commentaire)
#   - inline `# ...` strippé
#   - espaces (whitespace) trimées
if [ -f /etc/devcontainer-firewall/direct-tcp-allow.txt ]; then
  while IFS= read -r raw; do
    # Strip inline comment + trim whitespace
    entry=$(echo "$raw" | sed 's/#.*//' | tr -d '[:space:]')
    [ -z "$entry" ] && continue

    host=$(echo "$entry" | cut -d: -f1)
    port=$(echo "$entry" | cut -d: -f2)

    # Mot-clé spécial : "host" → host.docker.internal
    if [ "$host" = "host" ]; then
      ip="$HOST_IP"
    else
      ip=$(resolve_via_docker "$host")
    fi

    if [ -n "$ip" ]; then
      iptables -A OUTPUT -d "$ip" -p tcp --dport "$port" -j ACCEPT
      echo "📦 Direct TCP allow: $host ($ip):$port"
    else
      echo "⚠ $host not resolvable — skipped"
    fi
  done < /etc/devcontainer-firewall/direct-tcp-allow.txt
fi
```

Note importante : `CLAUDE_CODE_FIREWALL_DEBUG` reste lu de l'env si on
veut garder le toggle informatif. Pas sécurité-critique.

### Fichier 2f — Migration automatique dans `templates/v2/install.sh`

`install.sh` doit migrer les valeurs legacy `.env` → fichiers baked, de
manière idempotente, sans casser les projets existants :

```bash
ENV_FILE="$TARGET/.devcontainer/.env"
TCP_FILE="$TARGET/.devcontainer/firewall/direct-tcp-allow.txt"
MODE_FILE="$TARGET/.devcontainer/firewall/default-mode"

# === Migration 1 : .configured-firewall-mode → firewall/default-mode ===
LEGACY_MODE="$TARGET/.devcontainer/.configured-firewall-mode"
if [ -f "$LEGACY_MODE" ] && [ ! -s "$MODE_FILE" ]; then
  cp "$LEGACY_MODE" "$MODE_FILE"
  echo "→ migrated firewall mode : $LEGACY_MODE → firewall/default-mode"
fi

# === Migration 2 : CLAUDE_CODE_FIREWALL_ALLOWED → direct-tcp-allow.txt ===
if [ -f "$ENV_FILE" ] && grep -q '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$ENV_FILE"; then
  # Extraire la valeur (format historique : host:port,host:port,...)
  VALUE=$(grep '^CLAUDE_CODE_FIREWALL_ALLOWED=' "$ENV_FILE" | head -1 \
            | cut -d= -f2- | tr -d '"' | tr -d "'")

  if [ -n "$VALUE" ]; then
    # Append migration header + entries (un host:port par ligne)
    {
      echo ""
      echo "# Migrated from .env CLAUDE_CODE_FIREWALL_ALLOWED on $(date +%Y-%m-%d)"
      echo "$VALUE" | tr ',' '\n' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
    } >> "$TCP_FILE"
    echo "→ migrated $VALUE → firewall/direct-tcp-allow.txt"
  fi

  # Retirer la ligne de .env (idempotent)
  sed -i.bak '/^CLAUDE_CODE_FIREWALL_ALLOWED=/d' "$ENV_FILE"
  rm -f "$ENV_FILE.bak"
  echo "→ removed CLAUDE_CODE_FIREWALL_ALLOWED from .env (now baked)"
fi
```

Idempotent : si rerun (pas de `CLAUDE_CODE_FIREWALL_ALLOWED` dans
`.env`), rien ne se passe. Si le user a déjà migré manuellement, idem.

### Fichier 3 — `templates/v2/install.sh` (autres garanties)

**Garantir** que les fichiers/dossiers existent à l'install (sinon le
`COPY` échoue sur un projet vierge) :

```bash
# Section: setup_firewall_files()
touch "$TARGET/.devcontainer/firewall/domains.local.txt"
mkdir -p "$TARGET/.devcontainer/firewall/policy.local.d"
mkdir -p "$TARGET/.devcontainer/firewall/domains.d"
```

Idempotent — pas de garde nécessaire.

### Fichier 4 — `templates/v2/Dockerfile.base`

**Retirer** l'entrée sudoers pour `init-firewall.sh` (devenu inutile au
runtime) :

```dockerfile
# Avant :
RUN echo "$USERNAME ALL=(root) NOPASSWD: /usr/local/bin/init-firewall.sh, /usr/local/bin/test-firewall.sh" > /etc/sudoers.d/node-firewall

# Après :
RUN echo "$USERNAME ALL=(root) NOPASSWD: /usr/local/bin/test-firewall.sh" > /etc/sudoers.d/node-firewall
```

`test-firewall.sh` reste sudo'able car il est en lecture seule (ipset
test) et utile pour le diagnostic.

### Fichier 5 — `templates/v2/post-start.sh`

**Vérifier** que l'appel `sudo /usr/local/bin/init-firewall.sh` au boot
fonctionne encore. Le hook `postStartCommand` est exécuté par
`devcontainer-cli` qui **lance le shell sous `remoteUser` (node)**. Donc
le `sudo` à l'intérieur a besoin de l'entrée sudoers.

**Décision à prendre en début de session** :
- **Option A** — Garder l'entrée sudoers pour `init-firewall.sh` mais
  re-baker `init-firewall.sh` strict (option 5 de la session 2 — voir
  session suivante). C'est cohérent avec le bake-only.
- **Option B** — Déplacer l'init firewall vers un mécanisme qui run en
  root sans sudo (Docker entrypoint, ou `userCommands` dans
  `devcontainer.json`). Plus invasif.

Recommandation : **Option A**, simple. La session 2 sécurise
`init-firewall.sh` lui-même (le bug `/tmp/.firewall-env`).

### Fichier 6 — `.devcontainer/` (dogfooding mirror)

Copier les mêmes modifs vers `.devcontainer/`. Skip `Dockerfile.php`
(absent en dogfooding).

### Fichier 7 — `.devcontainer/SECURITY-AUDIT-2026-05.md`

**Créer** ce fichier avec le rapport d'audit (11 vecteurs) extrait de
`/home/node/.claude/plans/tu-es-un-expert-resilient-whale.md` (annexe).
Commit séparé du fix : « docs(security): add SECURITY-AUDIT-2026-05 ».

### Fichier 8 — `templates/v2/SECURITY.md`

**Créer** : threat model formalisé, invariants à maintenir, recipe
d'audit. Renvoyer vers `SECURITY-AUDIT-2026-05.md` pour le détail.

---

## Verification — comment confirmer que ça marche

Toutes ces commandes doivent être exécutées **dans le container après
rebuild**, en tant que `node`.

### 1. Les lectures continuent de marcher

```bash
cat /etc/devcontainer-firewall/domains.txt              # → contenu visible
cat /etc/devcontainer-firewall/policy.d/api.anthropic.com.yaml  # → contenu visible
ls /etc/devcontainer-firewall/addons/                   # → 5 fichiers .py
```

### 2. Les écritures sont bloquées

```bash
touch /etc/devcontainer-firewall/test-attack
# Attendu : touch: cannot touch '...': Read-only file system

echo "evil.com" >> /etc/devcontainer-firewall/domains.local.txt
# Attendu : zsh: read-only file system
```

### 3. Les écritures via /workspace ne se propagent PLUS à /etc

```bash
echo "evil.com" >> /workspace/.devcontainer/firewall/domains.local.txt
# Cette écriture réussit (workspace est rw, c'est normal et voulu)

grep -F "evil.com" /etc/devcontainer-firewall/domains.local.txt
# Attendu : AUCUN match. La copie dans l'image n'a pas l'addition.
```

C'est la preuve que les deux chemins sont maintenant **découplés**.

### 4. `sudo init-firewall.sh` est verrouillé (si Option B retenue)

```bash
sudo /usr/local/bin/init-firewall.sh
# Attendu : Sorry, user node is not allowed to execute ... as root.
```

Si Option A retenue, ce check n'est pas fait ici mais dans la session 2.

### 5. Le firewall fonctionne toujours au démarrage

```bash
# Après rebuild + container start :
sudo iptables -L OUTPUT -n | grep DROP
# Attendu : règles DROP présentes

curl -sSf https://google.com 2>&1 | head -3
# Attendu : erreur DNS ou connection refused (host non allowed)

curl -sSf https://api.anthropic.com/v1/models 2>&1 | head -3
# Attendu : 401 Unauthorized (= mitm a laissé passer, host allowed)
```

---

## Rollback — si quelque chose casse

Cette modif touche la chaîne de build. Si après merge le container ne
démarre plus :

```bash
# 1. Revert le commit du fix
git revert <hash>

# 2. Force rebuild du base layer pour purger les COPY non-désirées
docker rmi claude-devcontainer-base:$(grep '^CLAUDE_CODE_VERSION=' .devcontainer/.env | cut -d= -f2)

# 3. VS Code → "Dev Containers: Rebuild Container"
```

Les fichiers `.devcontainer/firewall/*` restent inchangés sur l'hôte
(jamais détruits par cette modif).

---

## Migration pour adopting projects

Les projets qui ont déjà adopté le devcontainer v2 doivent migrer
manuellement :

```bash
# 1. Récupérer les nouveaux templates
cp -p path/to/devcontainer-tools/templates/v2/docker-compose.yml .devcontainer/
cp -p path/to/devcontainer-tools/templates/v2/Dockerfile         .devcontainer/
cp -p path/to/devcontainer-tools/templates/v2/install.sh         .

# 2. Garantir les fichiers attendus par le nouveau Dockerfile
touch .devcontainer/firewall/domains.local.txt
mkdir -p .devcontainer/firewall/policy.local.d
mkdir -p .devcontainer/firewall/domains.d

# 3. Forcer le rebuild du base layer (Dockerfile.base a changé)
docker rmi claude-devcontainer-base:$(grep '^CLAUDE_CODE_VERSION=' .devcontainer/.env | cut -d= -f2)

# 4. VS Code → "Dev Containers: Rebuild Container"
```

Documenter cette recipe dans `LOG.md` à la fin de session.

---

## DoD (Definition of Done)

À la fin de la session :

1. **STATUS.md** : flip session 1 row 📋 → ✅, prompt → —, bump
   Delivered (0→1), Next focus = session 2.
2. **EXISTING.md** : tableau "Surface d'attaque" — passer vecteurs #1,
   #2, #3, #8 de 🔴 → 🟢. Vecteur #10 reste ⚪ (à confirmer session 6).
3. **LOG.md** : append `## 1 — immutable-firewall-config` daté
   2026-05-22+, avec files / What / Why / Decisions / Gotchas / Tests / Commit.
4. **Créer** `sessions/session-2-firewall-env-no-source.md` (déjà
   existant — vérifier qu'il est à jour avec les décisions prises ici,
   notamment Option A vs B).
5. **Proposer 2 commits** (NE PAS commit sans confirmation user) :
   - `docs(security): add SECURITY-AUDIT-2026-05 (11 vectors)`
   - `security: bake firewall config in image, drop runtime bind mount`

Messages self-contained (cf. CLAUDE.md §10).

---

## Prompt à coller dans une nouvelle session Claude

`````
Je démarre la session 1 du rollout `devcontainer-security-hardening`.

Entry point : `/workspace/plans/devcontainer-security-hardening/ROLLOUT.md`
Read also : STATUS.md, LOG.md, EXISTING.md, this file
            (sessions/session-1-immutable-firewall-config.md)

Reference spec (matériau de départ déjà rédigé) :
`plans/devcontainer-tools-v2-migration/sessions/part-1-session-3c-firewall-write-protection.md`

Goal de la session : implémenter le bake-only firewall (option B de
la spec de référence) + commit du rapport d'audit en préambule.

Suivre les sections "Implementation détaillée", "Verification" et "DoD"
du présent fichier. Décider en début de session : Option A vs B pour
le sudoers entry `init-firewall.sh` (recommandation : A, voir session 2
pour le détail). STOP + re-plan si blocker pendant l'étape 5.
`````
