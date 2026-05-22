# Session 4 — firewall-mode-baked-only (MERGED INTO SESSION 1)

> **⚠ STATUS** : **CONTENU MERGÉ dans session 1** (revue 2026-05-22).
> Le bake du fichier `firewall/default-mode` + suppression de la
> plomberie `FIREWALL_MODE` env → désormais partie intégrante de session 1
> ("bake-firewall-config" étendu). Idem pour `CLAUDE_CODE_FIREWALL_ALLOWED`
> → `firewall/allowed-hosts.txt`. Ce fichier reste comme **référence**
> du raisonnement et du PoC.

> **Effort original** : ~30 min | **Vecteur couvert** : sous-trou firewall-mode après sessions 1-2
> **Workflow préservé** : `firewall-mode.sh` continue d'exister mais signale qu'un rebuild est requis
> **Architecture** : 100% Dockerfile.base + retrait code dans init/post-start

## TL;DR — pour les pressés

Après sessions 1-2, il reste UN sous-trou : `node` peut faire passer le
firewall en mode `off` (kill-switch légitime) via le fichier
`.configured-firewall-mode` du workspace, puis un reload désactive le
firewall.

Cette session ferme le trou avec une approche **minimale et propre** :
- Le mode firewall est **baké** dans l'image (`/etc/devcontainer-firewall/default-mode`,
  par défaut `strict`)
- `init-firewall.sh` lit ce fichier baké, **point**
- Plus de passage par `/tmp/.firewall-env`, plus de `.env`, plus de
  workspace flag file lu au runtime
- Pour changer le mode → **rebuild**

C'est la même contrainte que pour le reste de la config firewall
(session 1) : modifier = rebuild. Conceptuellement cohérent.

---

## Pourquoi cette session existe

### Threat model assumé

Le critère sandboxing de `node` :
1. Ne peut pas relancer la machine seul ✓
2. Ne peut pas **modifier le firewall sans rebuild** ← TROU sans cette session
3. Ne peut pas exfiltrer sans rebuild

Le mode firewall est **un paramètre du firewall**. Si `node` peut le
toggler à `off`, il modifie effectivement le firewall sans rebuild → viole
le critère 2.

### Pourquoi cette session est minimale

L'approche "baked-only" est conceptuellement identique à session 1
(la config firewall vit dans l'image, modif = rebuild). Le mode n'est
qu'un fichier de plus dans `firewall/`. Le bake de session 1 le pickera
automatiquement.

Effort réel : retirer du code (élimine la plomberie `.configured-firewall-mode`
→ `firewall-env-write FIREWALL_MODE=...` → `/var/run/.../env`). C'est
un **net negative LOC**.

---

## Le problème — version humaine

### PoC actuel (avant fix)

```bash
# 1. Activer le mode off
echo "off" > /workspace/.devcontainer/.configured-firewall-mode

# 2. Reload (Reopen Folder in VS Code)

# 3. post-start.sh lit "off" depuis le workspace
# 4. l'envoie à firewall-env-write FIREWALL_MODE=off
# 5. init-firewall.sh honore off → flush iptables
# 6. Firewall désactivé. Exfil libre.
```

### Pourquoi le mode off existe

`firewall-mode.sh off` est un kill-switch légitime documenté
(debug réseau, désactivation en cas de bug mitmproxy). MAIS son usage
est admin, rare, et toujours conscient. Donc :
- L'opération est délibérée → rebuild est acceptable
- Doit demander une action explicite, pas un simple toggle workspace

---

## La solution

### Principe

**Le mode firewall vit dans un fichier baké**, comme le reste de la
config firewall.

```
/etc/devcontainer-firewall/default-mode    (root:root 0644, baked)
```

Contenu : `strict`, `basic`, ou `off` (chaîne unique).

`init-firewall.sh` lit ce fichier. C'est tout. Plus de variable d'env,
plus de fichier workspace.

### Comment changer le mode

Modifier `.devcontainer/firewall/default-mode` (dans le workspace,
canonical source) + **rebuild**. Session 1 bake tout `firewall/` →
le fichier est automatiquement inclus.

`firewall-mode.sh` est conservé mais simplifié :

```bash
#!/usr/bin/env bash
# firewall-mode.sh — set the default firewall mode for next build.
set -euo pipefail
MODE="${1:-}"
case "$MODE" in
  strict|basic|off) ;;
  *) echo "Usage: $0 strict|basic|off" >&2; exit 1 ;;
esac

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
echo "$MODE" > "$SCRIPT_DIR/firewall/default-mode"

cat <<EOF
✓ Mode '$MODE' written to .devcontainer/firewall/default-mode
  This file is baked into the image at build time (session 1 firewall bake).
  → Rebuild the container in VS Code to apply.
EOF
```

Effet : modifier le mode = modifier un fichier `firewall/` + rebuild.
Identique au workflow "modifier `domains.txt`" — cohérent.

### Pourquoi ne PAS passer par `.env`

L'user a explicitement écarté `.env` pour éviter de confondre :
- Config build-time (image content, immutable) ↔ fichier `firewall/`
- Config runtime (env vars passées à PID 1, mutable au reload) ↔ `.env`

Le mode firewall est **build-time** (immuable au runtime). Donc dans un
fichier `firewall/`, pas dans `.env`. Cette distinction garde `.env`
sémantiquement clean pour les vraies vars d'environnement runtime.

### Retraits dans le code existant

Cette session ne fait que **retirer du code** :

| Fichier | Retrait |
|---|---|
| `post-start.sh` | Suppression du `FW_MODE=$(cat .configured-firewall-mode...)` + appel `firewall-env-write FIREWALL_MODE=...` |
| `on-create.sh` | Idem |
| `init-firewall.sh` | Suppression du `case "${FIREWALL_MODE:-strict}"` qui lisait la var d'env ; lit directement le fichier baké |
| `firewall-env-write` | Retrait de `FIREWALL_MODE` des clés acceptées |
| `firewall-mode.sh` | Simplification massive (10 lignes au lieu de 110) |
| Legacy : `.configured-firewall-mode` | Reste dans le workspace pour rétro-compat ; ignored |

---

## Impact sur le workflow dev

### Ce qui CHANGE

| Action | Avant | Après |
|---|---|---|
| `firewall-mode.sh strict` puis reload | OK | `firewall-mode.sh strict` + **Rebuild** |
| `firewall-mode.sh basic` puis reload | OK | idem + Rebuild |
| `firewall-mode.sh off` puis reload | OK | idem + Rebuild |

### Ce qui NE CHANGE PAS

- ✅ Le mode strict (95% des cas) reste le default — projet vierge n'a
  rien à faire.
- ✅ `firewall-mode.sh` continue d'exister (juste fait edit + signal rebuild).
- ✅ Le concept "modifier la config firewall = rebuild" est appliqué de
  manière uniforme (session 1 + cette session).

### Workflow pour le user qui veut juste tester

```bash
# Tester un host normalement bloqué
echo 'mon-service.example.com' >> .devcontainer/firewall/domains.local.txt
# Rebuild
# → host accessible

# Désactiver complètement le firewall pour un debug réseau
.devcontainer/firewall-mode.sh off
# Rebuild
# → firewall off
# Quand le debug est fini :
.devcontainer/firewall-mode.sh strict
# Rebuild
```

Workflow uniforme, sans piège.

---

## Implementation détaillée

### Fichier 1 — `templates/v2/firewall/default-mode` (nouveau)

Créer le fichier :
```
strict
```

C'est tout. Une seule ligne. Session 1 (bake firewall) le COPYra
automatiquement avec le reste de `firewall/`.

### Fichier 2 — `templates/v2/install.sh`

Garantir que le fichier existe avant build (sinon COPY fail) :

```bash
[ -f "$TARGET/.devcontainer/firewall/default-mode" ] || \
    echo "strict" > "$TARGET/.devcontainer/firewall/default-mode"
```

Idempotent. Une ligne.

### Fichier 3 — `templates/v2/init-firewall.sh`

**Retirer** le bloc qui lisait `FIREWALL_MODE` depuis env :

```bash
# Avant — supprimer :
# case "${FIREWALL_MODE:-strict}" in
#   paranoid) FIREWALL_MODE=strict ;;
#   okeish)   FIREWALL_MODE=basic  ;;
# esac
# FIREWALL_MODE="${FIREWALL_MODE:-strict}"

# Après — lire depuis fichier baké :
FIREWALL_MODE=$(cat /etc/devcontainer-firewall/default-mode 2>/dev/null || echo strict)
# Légère validation au cas où le fichier est corrompu / vide
case "$FIREWALL_MODE" in
  strict|basic|off|paranoid|okeish) ;;  # accept legacy aliases
  *) FIREWALL_MODE=strict ;;             # fail-safe
esac
case "$FIREWALL_MODE" in
  paranoid) FIREWALL_MODE=strict ;;
  okeish)   FIREWALL_MODE=basic  ;;
esac
```

### Fichier 4 — `templates/v2/post-start.sh`

**Retirer** :

```bash
# Suppression :
# FW_MODE=$(cat /workspace/.devcontainer/.configured-firewall-mode 2>/dev/null || echo strict)
# echo "FIREWALL_MODE=$FW_MODE" >> /tmp/.firewall-env  (legacy)
# OU : sudo firewall-env-write "FIREWALL_MODE=$FW_MODE"  (post-session-2)
```

`firewall-env-write` n'est plus appelé avec `FIREWALL_MODE`. Garder
l'appel pour `CLAUDE_CODE_FIREWALL_ALLOWED` et `CLAUDE_CODE_FIREWALL_DEBUG`
si ces vars sont définies.

Optionnel : ajouter un log informatif :
```bash
echo "ℹ Firewall mode (baked) : $(cat /etc/devcontainer-firewall/default-mode)"
```

### Fichier 5 — `templates/v2/on-create.sh`

Idem post-start.sh.

### Fichier 6 — `templates/v2/firewall/firewall-env-write` (session 2)

Retirer `FIREWALL_MODE` de la `case "$key"` allowlist :

```bash
case "$key" in
  CLAUDE_CODE_FIREWALL_ALLOWED|CLAUDE_CODE_FIREWALL_DEBUG) ;;
  # FIREWALL_MODE retiré — mode is baked, not runtime-set
  *)
    echo "⚠ rejected unknown key: $key" >&2
    continue
    ;;
esac
```

### Fichier 7 — `templates/v2/firewall-mode.sh`

Simplifier massivement :

```bash
#!/usr/bin/env bash
# firewall-mode.sh — set the default firewall mode for next build.
# To apply the new mode : rebuild the container in VS Code.
set -euo pipefail

MODE="${1:-}"
case "$MODE" in
  strict|basic|off) ;;
  -h|--help|"")
    cat <<EOF
Usage : $0 strict|basic|off

  strict  DNS allowlist + mitmproxy enforcement (default)
  basic   DNS allowlist only (no mitm)
  off     no filter (kill-switch — admin only)

The mode is baked into the image at build time. Run this script then
"Rebuild Container" in VS Code to apply.
EOF
    exit 0 ;;
  *) echo "Invalid mode: $MODE" >&2; exit 1 ;;
esac

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
echo "$MODE" > "$SCRIPT_DIR/firewall/default-mode"

echo "✓ Mode '$MODE' written to .devcontainer/firewall/default-mode"
echo "→ Rebuild the container in VS Code to apply."
```

### Fichier 8 — `.devcontainer/` (dogfooding mirror)

Copier toutes les modifs équivalentes.

### Fichier 9 — `templates/v2/SECURITY.md`

Documenter le workflow et le pourquoi.

---

## Verification

### 1. PoC firewall-off DOIT échouer (sans rebuild)

```bash
# Tentative legacy
echo "off" > /workspace/.devcontainer/.configured-firewall-mode
# Au reload, ce fichier est IGNORED. firewall reste strict.

docker compose restart
# Attendre boot

sudo iptables -L OUTPUT -n | grep DROP | head -3
# Attendu : règles DROP présentes
cat /etc/devcontainer-firewall/default-mode
# Attendu : "strict"
```

### 2. Workflow légitime fonctionne

```bash
# Switcher en basic
.devcontainer/firewall-mode.sh basic
# → "✓ Mode 'basic' written..."

# Rebuild (VS Code)
# Après rebuild :
cat /etc/devcontainer-firewall/default-mode  # → basic
sudo iptables -L OUTPUT -n  # → ipset ACCEPT sans -m owner uid filter
```

### 3. `firewall-env-write` rejette FIREWALL_MODE

```bash
sudo /usr/local/bin/firewall-env-write "FIREWALL_MODE=off"
# Attendu : "⚠ rejected unknown key: FIREWALL_MODE"
```

### 4. Pas de plumberie résiduelle

```bash
grep -RE "FIREWALL_MODE" /workspace/.devcontainer/ \
  --exclude-dir=logs --exclude-dir=firewall 2>/dev/null
# Attendu : Aucun match dans post-start.sh / on-create.sh
# (sauf init-firewall.sh qui le lit du fichier baked + l'utilise localement)
```

---

## Edge cases

### EC1 — Projet adopting avec `.configured-firewall-mode = basic`

Au build (post-migration), `install.sh` migre :

```bash
# Dans install.sh :
if [ -f "$TARGET/.devcontainer/.configured-firewall-mode" ] && \
   [ ! -s "$TARGET/.devcontainer/firewall/default-mode" ]; then
  cp "$TARGET/.devcontainer/.configured-firewall-mode" \
     "$TARGET/.devcontainer/firewall/default-mode"
  echo "→ migrated mode from .configured-firewall-mode to firewall/default-mode"
fi
```

### EC2 — Confusion utilisateur "j'ai édité, ça marche pas"

L'user édite `.devcontainer/firewall/default-mode` directement (au lieu
de `firewall-mode.sh`) et oublie de rebuild. Pas de changement à chaud,
mais c'est l'objectif. Le banner `post-start.sh` peut afficher le mode
courant pour faciliter le diagnostic :

```
ℹ Firewall mode (baked) : strict
```

Le user voit que ce n'est pas son mode attendu → comprend qu'il a oublié
le rebuild.

---

## Rollback

```bash
git revert <hash>
docker rmi claude-devcontainer-base:$(grep '^CLAUDE_CODE_VERSION=' .devcontainer/.env | cut -d= -f2)
# Rebuild
```

---

## DoD

1. **STATUS.md** : flip 4 row 📋 → ✅, prompt → —, Delivered 3→4, Next = session 5 ou 6 selon reclassification.
2. **EXISTING.md** : ajouter "Toggle firewall mode" → 🟢. Documenter
   explicitement le threat model "node sandboxé" dans une nouvelle
   section.
3. **LOG.md** : append `## 4 — firewall-mode-baked-only` daté.
4. **Créer / vérifier** session suivante.
5. **Proposer commit** :
   `security: firewall mode is now baked-only, no runtime path can downgrade it`

---

## Prompt à coller

`````
Je démarre la session 4 du rollout `devcontainer-security-hardening`.

Entry point : `/workspace/plans/devcontainer-security-hardening/ROLLOUT.md`
Read : STATUS.md, LOG.md, EXISTING.md, this file
       (sessions/session-4-firewall-mode-baked-only.md).

IMPORTANT — approche minimale. Pas de bake de scripts lifecycle. Pas de
.env. Just :
1. Bake un fichier `/etc/devcontainer-firewall/default-mode` (par défaut
   "strict"). Session 1 le COPY automatiquement avec le reste de firewall/.
2. init-firewall.sh lit ce fichier baké, point.
3. Retirer FIREWALL_MODE de la plomberie env (firewall-env-write,
   post-start.sh, on-create.sh).
4. firewall-mode.sh devient un edit-file + signal rebuild.

Net LOC : négatif (suppression > addition).

Renommer fichier : sessions/session-4-bind-mount-script-validation.md
→ sessions/session-4-firewall-mode-baked-only.md (`git mv`).

Suivre "Implementation détaillée" + "Verification" + "DoD" + EC1 migration.
`````
