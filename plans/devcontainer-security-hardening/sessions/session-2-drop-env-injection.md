# Session 2 — drop-env-injection

> **Effort** : ~1 h | **Vecteurs couverts** : #4 (privesc `/tmp/.firewall-env`)
> **Workflow préservé** : aucun impact pour le dev — pure suppression de plomberie obsolète
> **Architecture** : net LOC négatif — on supprime du code, on n'en ajoute pas

## TL;DR — pour les pressés

Après session 1 (bake de toute la config firewall incluant mode +
allowed-hosts), il n'y a **plus aucune raison** de passer des variables
firewall via fichier intermédiaire `/tmp/.firewall-env`. Cette session
**supprime** :
- La ligne `source /tmp/.firewall-env` dans `init-firewall.sh` (privesc
  vector #4)
- L'écriture de ce fichier dans `post-start.sh` et `on-create.sh`
- Le concept de helper `firewall-env-write` qui n'a plus rien à
  transmettre

Résultat : moins de code, plus simple, et la privesc root via
world-writable est éliminée par construction.

---

## Le problème — version humaine

### Métaphore

Une voiture qui transporte un message d'un service à un autre. Au début
(session originale), le message était laissé sur un post-it dans le hall
(`/tmp/.firewall-env`), avec un risque d'être réécrit par n'importe qui.
La session 2 originale proposait de mettre le message dans une boîte
verrouillée (`/var/run/devcontainer-firewall/env`, root-only).

Mais après session 1, **le message n'a plus de raison d'exister** — le
destinataire (init-firewall.sh) lit directement la source canonique
(les fichiers baked dans l'image). La voiture, la boîte, le post-it :
tout disparaît.

### Privesc original (rappel)

```bash
# init-firewall.sh ligne 7 (avant fix) :
[ -f /tmp/.firewall-env ] && source /tmp/.firewall-env

# /tmp est world-writable. Le source = exécution. init-firewall.sh tourne
# en root via sudo NOPASSWD. Donc n'importe quel code dans /tmp/.firewall-env
# s'exécute en root.

# PoC en 3 commandes :
echo 'echo node ALL=(root) NOPASSWD: ALL >> /etc/sudoers' > /tmp/.firewall-env
sudo /usr/local/bin/init-firewall.sh
sudo -i  # → root persistant
```

### Pourquoi la suppression simple (vs whitelist parser)

L'approche initiale de session 2 était : remplacer `source` par un
parser whitelist sur un fichier root-only. C'est une bonne défense en
profondeur, mais après session 1, c'est **complexité gratuite** : si
les seules variables qu'on injectait étaient `FIREWALL_MODE` et
`CLAUDE_CODE_FIREWALL_ALLOWED`, et qu'elles sont maintenant baked, il
n'y a plus rien à injecter.

Donc : **supprime la ligne, supprime le mécanisme**. Plus simple, plus
sûr.

---

## La solution

### Ce qui est supprimé

| Fichier | Suppression |
|---|---|
| `init-firewall.sh` | Ligne 7 : `[ -f /tmp/.firewall-env ] && source /tmp/.firewall-env` |
| `test-firewall.sh` | Ligne similaire (à vérifier) |
| `post-start.sh` | Bloc qui écrit `/tmp/.firewall-env` (3 lignes) |
| `on-create.sh` | Idem |
| `firewall-env-write` | Helper entier supprimé (s'il avait été créé en session 2 v0 — sinon n'a jamais existé) |
| sudoers entry pour `firewall-env-write` | Retiré si présent |

### Ce qui reste

`CLAUDE_CODE_FIREWALL_DEBUG` est le seul flag non sécurité-critique qui
peut rester dans `.env`. Il contrôle juste la verbosité de
`init-firewall.sh`. Lu via env env-var classique (docker-compose
env_file → PID 1 → child processes inheritance), pas besoin de helper.

### Vérification que rien n'est cassé

`init-firewall.sh` doit fonctionner sans aucune variable passée en
provenance de node. Il lit :
- `/etc/devcontainer-firewall/default-mode` (mode, baked)
- `/etc/devcontainer-firewall/allowed-hosts.txt` (hosts directs, baked)
- `CLAUDE_CODE_FIREWALL_DEBUG` depuis l'env classique (non-critique)

Tout le reste est lu de la baked config (session 1).

---

## Impact sur le workflow dev

### Ce qui CHANGE

Rien de visible pour le user. Purement défensif.

### Ce qui NE CHANGE PAS

- ✅ Le mode firewall continue de fonctionner (mode strict baked par défaut)
- ✅ Les hosts directs (claude-bridge:9223 etc.) continuent d'être ajoutés
- ✅ La verbosité debug continue de marcher via `.env`

---

## Implementation détaillée

### Fichier 1 — `templates/v2/init-firewall.sh`

```bash
# Supprimer ligne 7 et son commentaire :
# # Load env vars passed from post-start.sh (sudo blocks env passthrough)
# [ -f /tmp/.firewall-env ] && source /tmp/.firewall-env
```

(Et la lecture `FIREWALL_MODE` / `CLAUDE_CODE_FIREWALL_ALLOWED` depuis
env est remplacée par la lecture des fichiers baked — déjà fait en
session 1.)

### Fichier 2 — `templates/v2/test-firewall.sh`

Idem : retirer le `source /tmp/.firewall-env` (ligne 23 dans le fichier
actuel).

### Fichier 3 — `templates/v2/post-start.sh`

Supprimer le bloc :

```bash
# AVANT — supprimer :
# echo "CLAUDE_CODE_FIREWALL_ALLOWED=${CLAUDE_CODE_FIREWALL_ALLOWED:-}" > /tmp/.firewall-env
# echo "CLAUDE_CODE_FIREWALL_DEBUG=${CLAUDE_CODE_FIREWALL_DEBUG:-}" >> /tmp/.firewall-env
# echo "FIREWALL_MODE=$FW_MODE" >> /tmp/.firewall-env
```

Si on garde `CLAUDE_CODE_FIREWALL_DEBUG` : il est déjà dans
l'environnement de `post-start.sh` (via env_file → PID 1 →
post-start.sh hérite). On peut le passer à `init-firewall.sh` via :

```bash
# Option A — sudo -E avec env_keep dans sudoers (le moins invasif) :
sudo --preserve-env=CLAUDE_CODE_FIREWALL_DEBUG /usr/local/bin/init-firewall.sh

# Option B — passer en argument CLI explicite :
sudo /usr/local/bin/init-firewall.sh --debug
```

Option B est plus propre (pas de env passthrough). À choisir en début
de session.

### Fichier 4 — `templates/v2/on-create.sh`

Pareil — supprimer le bloc d'écriture `/tmp/.firewall-env`.

### Fichier 5 — `templates/v2/Dockerfile.base`

Vérifier qu'on ne laisse pas une sudoers entry pour `firewall-env-write`
qui n'existe pas :

```dockerfile
# Sudoers — garder uniquement init-firewall.sh + test-firewall.sh
RUN echo "$USERNAME ALL=(root) NOPASSWD: /usr/local/bin/init-firewall.sh, /usr/local/bin/test-firewall.sh" > /etc/sudoers.d/node-firewall
```

(Si une session précédente avait ajouté `firewall-env-write`, le retirer.)

### Fichier 6 — `.devcontainer/` (dogfooding mirror)

Copier les modifs équivalentes.

---

## Verification

### 1. PoC privesc DOIT échouer

```bash
echo 'touch /tmp/pwned-as-root' > /tmp/.firewall-env
sudo /usr/local/bin/init-firewall.sh

# Vérifier
ls /tmp/pwned-as-root 2>&1
# Attendu : "No such file or directory"
# (init-firewall.sh ne source plus le fichier)
```

### 2. Le firewall démarre proprement

```bash
docker compose restart
# Attendre boot, puis :

sudo iptables -L OUTPUT -n | grep DROP | head -3
# Attendu : règles DROP présentes (mode strict baked)

cat /etc/devcontainer-firewall/default-mode
# Attendu : "strict"

sudo ipset list allowed-domains | head -5
# Attendu : entrées populées par dnsmasq
```

### 3. Pas de plomberie résiduelle

```bash
ls /tmp/.firewall-env 2>&1
# Attendu : "No such file or directory" (n'est plus créé)

# Aussi vérifier que /var/run/devcontainer-firewall/env n'est pas créé
# (vestige de la design intermédiaire session 2 v0)
ls /var/run/devcontainer-firewall/env 2>&1
# Attendu : "No such file or directory"

# Et pas de helper résiduel
ls /usr/local/bin/firewall-env-write 2>&1
# Attendu : "No such file or directory"

# Et pas de référence dans sudoers
sudo -n -l
# Attendu : juste init-firewall.sh + test-firewall.sh
```

### 4. CLAUDE_CODE_FIREWALL_DEBUG passe toujours

```bash
# Dans .env :
echo "CLAUDE_CODE_FIREWALL_DEBUG=true" >> /workspace/.devcontainer/.env

# Reload, puis observer post-start.log :
grep "dbg\|debug" /workspace/.devcontainer/logs/post-start-*.log
# Attendu : lignes verbose visibles
```

---

## Edge cases

### EC1 — Une session v0 a créé `firewall-env-write`

Si une version intermédiaire du rollout a créé le helper, le supprimer :
- `git rm templates/v2/firewall/firewall-env-write`
- `git rm .devcontainer/firewall/firewall-env-write`
- Retirer le COPY associé dans `Dockerfile.base`
- Retirer la sudoers entry

### EC2 — Sudo strip env de `CLAUDE_CODE_FIREWALL_DEBUG`

Effectivement : `sudo init-firewall.sh` ne propage pas l'env par défaut.
Solutions :
- Option A (sudo --preserve-env) — simple mais ajoute env passthrough
- Option B (CLI flag `--debug`) — plus propre

Recommandation : **Option B**, c'est minimal et explicite.

```bash
# post-start.sh :
DEBUG_FLAG=""
[ "${CLAUDE_CODE_FIREWALL_DEBUG:-}" = "true" ] && DEBUG_FLAG="--debug"
sudo /usr/local/bin/init-firewall.sh $DEBUG_FLAG

# init-firewall.sh :
DEBUG=false
[ "${1:-}" = "--debug" ] && DEBUG=true
```

---

## Rollback

```bash
git revert <hash>
# Pas de purge image nécessaire — cette session ne touche pas
# Dockerfile.base content baked, juste les scripts.
docker compose restart
```

---

## DoD

1. **STATUS.md** : flip 2 row 📋 → ✅, prompt → —, Delivered N→N+1.
2. **EXISTING.md** : vecteur #4 passe 🔴 → 🟢.
3. **LOG.md** : append `## 2 — drop-env-injection` daté, expliquer le
   scope simplifié (suppression au lieu de remplacement par whitelist).
4. **Proposer commit** :
   `security: remove /tmp/.firewall-env injection — config now read from baked files (session 1)`

---

## Prompt à coller

`````
Je démarre la session 2 du rollout `devcontainer-security-hardening`.

Entry point : `/workspace/plans/devcontainer-security-hardening/ROLLOUT.md`
Read : STATUS.md, LOG.md, EXISTING.md, this file
       (sessions/session-2-drop-env-injection.md).

Pre-req : session 1 (bake firewall config) DOIT être délivrée. Cette
session est purement un cleanup post-session-1.

Goal : supprimer la plomberie `/tmp/.firewall-env` (et helpers
associés s'ils existent). Net LOC négatif.

Suivre "Implementation détaillée" + "Verification" + "DoD".

Décision en début de session :
- EC2 : Option A (sudo --preserve-env) vs Option B (CLI --debug flag) ?
  Recommandation : B.
- EC1 : check si firewall-env-write existe et le purger si oui.

Renommer fichier : sessions/session-2-firewall-env-no-source.md
→ sessions/session-2-drop-env-injection.md (`git mv`).
`````
