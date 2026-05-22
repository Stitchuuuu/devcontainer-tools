# Part 1 — session 3c — firewall-write-protection

> **Effort** : ~1-2 h (option A — `:ro` overlay) | ~3-4 h (option B —
> bake-only) | **Dependencies** : Part 1 session 3 (firewall-layer-split)
> delivered. Indépendante de 3b et 4 — peut partir en parallèle ou
> dans n'importe quel ordre. Découverte pendant la review post-S3.

## Why this session

**Threat model** : Claude (utilisateur `node` dans le devcontainer)
peut actuellement modifier n'importe quel fichier firewall via le
workspace bind mount writable. Au prochain restart user-initiated
(post-start.sh → `sudo init-firewall.sh`), les modifications sont
picked up et la policy compromise est appliquée.

**Attack chain :**

1. Claude écrit dans `/workspace/.devcontainer/firewall/domains.local.txt`
   → ajoute `[GET,POST] *` (ou n'importe quoi)
2. Mid-session, `sudo init-firewall.sh` mid-flight = self-DoS (le
   script est one-shot, drop l'état iptables avant rebuild). Donc
   l'attaque n'est PAS immédiate.
3. **MAIS** au prochain `Rebuild Container` / start/stop/start
   user-initiated, `post-start.sh` lance `sudo init-firewall.sh` qui
   pick up le fichier modifié → exfiltration possible

**Variante dnsmasq plus furtive :** au lieu d'élargir la policy,
Claude édite `dnsmasq.conf` pour ajouter
`address=/api.anthropic.com/<attacker_ip>` — hijack DNS d'un host
**déjà allowlisté**. Même timeline.

**Scope du blast :** tout `.devcontainer/firewall/*` est sensible :
`domains.txt`, `domains.local.txt`, `policy.d/`, `policy.local.d/`,
`dnsmasq.conf`, `addons/*.py`, `tests/*`.

**Pourquoi `:ro` actuel ne protège pas :** le mount
`./firewall:/etc/devcontainer-firewall:ro` (docker-compose.yml:48)
bloque les écritures **via ce mount-là**. Mais les mêmes inodes host
sont accessibles writable via `/workspace/.devcontainer/firewall/*`
(le workspace mount `..:/workspace:delegated`). Écrire via workspace
modifie le fichier host → la vue `:ro` à `/etc/` voit le fichier
modifié au prochain read.

**Pourquoi P1-S3 n'a rien changé sécurité-wise :** les COPYs build-time
du Dockerfile sont shadowed au runtime par le bind mount. Le COPY
ne sert qu'au fallback hors docker-compose (raw `docker run`,
prepare-research bundle). Au runtime normal, c'est le bind mount qui
pilote tout.

## Decision : two options to choose from

### Option A — `:ro` overlay (minimal, preserve UX)

Ajouter une seconde ligne dans `docker-compose.yml` qui re-bind
`.devcontainer/firewall/` en read-only au path
`/workspace/.devcontainer/firewall/`, shadowing la writable parent
mount au sous-arbre firewall. Docker traite les mounts par
spécificité : le mount le plus profond gagne pour son sous-arbre.

```yaml
volumes:
  - ..:/workspace:delegated
+ - ./firewall:/workspace/.devcontainer/firewall:ro
  - ./firewall:/etc/devcontainer-firewall:ro
```

**Pros :**
- 1 ligne YAML
- UX "edit host → restart → reload" préservée intacte
- Backward-compatible avec adopting projects (juste re-`install.sh`
  pour pick up le docker-compose.yml updated)

**Cons :**
- Les fichiers restent live au runtime (bind mount actif) — donc
  un user éditant l'host file peut modifier la policy au prochain
  restart. Acceptable si on considère que l'attaquant est Claude,
  pas l'utilisateur lui-même.

**Effort :** ~1-2 h (édit + mirror + LOG + KNOWLEDGE + tests).

### Option B — Bake-only (immutable firewall config in image)

Supprimer le bind mount `./firewall:/etc/devcontainer-firewall:ro`
entièrement. Tous les fichiers firewall vivent baked dans l'image
(via Dockerfile COPY). Au runtime, `/etc/devcontainer-firewall/`
voit l'image content, pas l'host directory.

Conséquences :
- Edit firewall config = rebuild requis (~5s project layer pour les
  4 fichiers project, ~5-8 min base si dnsmasq.conf/addons/tests
  changent)
- Aucune surface d'attaque runtime : tout est immutable dans l'image
- `install.sh` doit garantir que `domains.local.txt` + `policy.local.d/`
  existent avant le build (sinon COPY fail), même si empty

**Pros :**
- Sécurité maximale : zéro vecteur d'écriture runtime
- Mental model simple : "firewall config est partie de l'image"
- Élimine le besoin de `sudoers.d/node-firewall` (init-firewall.sh
  ne sert plus à reload — déjà one-shot anyway)

**Cons :**
- Edit firewall = rebuild (~5s à ~8min selon ce qui change)
- `install.sh` change (touch + mkdir avant build)
- Project layer Dockerfile gère plus de COPYs (+ `domains.local.txt`
  + `policy.local.d/`)
- Adopting projects doivent se ré-installer + rebuild

**Effort :** ~3-4 h (édits multiples + install.sh + tests +
docs + migration recipe pour adopting projects).

### Recommandation

**Option B** si le threat model "Claude untrusted" est sérieux et
l'objectif est "zéro accès root / config immutable côté Claude"
(comme énoncé par l'user le 2026-05-22 : *"cadenas du côté du
devcontainer, sans accès root"*). Le coût UX (rebuild pour edit) est
absorbable vu que les edits firewall sont rares.

**Option A** si on veut un fix incremental "minimal viable security",
quitte à revisiter plus tard. Bloque l'attaque Claude immédiate sans
toucher l'UX.

À discuter avec l'user en début de session — option B est le path
recommandé.

## Where this session runs

Édit canonique : `templates/v2/` au root du repo `devcontainer-tools`
(ou son alias host `devcontainer-tools-v2/`). Si le repo a son propre
`.devcontainer/` pour dogfooding, mirror via `cp` (même pattern que
P1-S3).

Adopting projects (cyro-live, portal42, etc.) inherit le fix au
prochain `install.sh` rerun + rebuild container. Migration recipe
côté adopting projects à inclure (option B nécessite un rebuild ; A
nécessite un container recreate sans rebuild image).

## Prompt to paste

`````
Je démarre la Part 1 session 3c (firewall-write-protection) du rollout
`devcontainer-tools-v2-migration`.

Entry point : `plans/devcontainer-tools-v2-migration/ROLLOUT.md`
Read also :
- `STATUS.md` (Part 1 progress)
- `LOG.md` § P1-S3 (firewall layer split context)
- `KNOWLEDGE.md` § "Image layer split" (build-vs-runtime explained)
- `sessions/part-1-session-3c-firewall-write-protection.md` (this spec)

Goal : empêcher Claude (user `node` dans le container) de modifier
n'importe quel fichier firewall via le workspace bind mount writable.
Le `:ro` mount actuel à `/etc/devcontainer-firewall/` ne suffit pas
car les mêmes inodes host sont accessibles writable via
`/workspace/.devcontainer/firewall/*`. Objectif : start/stop/start
classique = re-applique la config user-authored, jamais Claude-tampered.

Étape 0 : confirmer avec l'user l'option à implémenter (A = `:ro`
overlay, B = bake-only). Recommandation default = B (per le requirement
"cadenas devcontainer, sans accès root" énoncé 2026-05-22).

Session 3c scope (option B — bake-only) :

1. **Edit `templates/v2/docker-compose.yml`** : supprimer la ligne
   `./firewall:/etc/devcontainer-firewall:ro`. Le `/etc/...` du
   container vient uniquement de l'image baked.

2. **Edit `templates/v2/Dockerfile`** + `templates/v2/Dockerfile.php` :
   ajouter COPY pour les 4 fichiers project-level qui étaient
   précédemment fournis par le bind mount (et déjà partiellement
   COPY'd par P1-S3) :
   - `firewall/domains.txt`           → /etc/devcontainer-firewall/
   - `firewall/domains.local.txt`     → /etc/devcontainer-firewall/
   - `firewall/policy.d/`             → /etc/devcontainer-firewall/policy.d/
   - `firewall/policy.local.d/`       → /etc/devcontainer-firewall/policy.local.d/
   La RUN `firewall-docker-setup.sh` reste inchangée (perms idempotent).

3. **Edit `templates/v2/install.sh`** : garantir que `domains.local.txt`
   et `policy.local.d/` existent à l'install time (sinon le COPY du
   project Dockerfile fail). Section `setup_firewall_layer()` ou
   équivalent :
   ```bash
   touch "$TARGET/.devcontainer/firewall/domains.local.txt"
   mkdir -p "$TARGET/.devcontainer/firewall/policy.local.d"
   ```
   Idempotent.

4. **Edit `templates/v2/Dockerfile.base`** : retirer
   l'entrée `sudoers.d/node-firewall` pour init-firewall.sh (devenue
   inutile vu que init-firewall.sh est one-shot au boot et que
   l'image est désormais immutable). Garder `test-firewall.sh` (lecture
   read-only, OK pour Claude).

5. **Edit `templates/v2/post-start.sh`** : si elle re-trigger
   init-firewall.sh, vérifier que ça marche encore sans la sudoers
   entry. Si non, lifecycle hook doit run en root déjà (à confirmer).

6. **Mirror dans dogfooding `.devcontainer/`** (via `cp`) :
   - `.devcontainer/docker-compose.yml`
   - `.devcontainer/Dockerfile`
   - `.devcontainer/Dockerfile.base`
   - Skip `Dockerfile.php` (absent en dogfooding)

7. **Update SCOPE.md, KNOWLEDGE.md** : nouvelle section "Security
   model — firewall immutability" expliquant le shift bind-mount →
   baked, la dependency `install.sh` ↔ Dockerfile COPY, et le
   tradeoff UX (rebuild required).

8. **Create `templates/v2/SECURITY.md`** : threat model formalisé +
   invariants à maintenir + recipe pour audit. Mentionner les
   surfaces hors-firewall encore à investiguer (.claude/,
   .claude-creds/, mitmproxy volume).

9. **Migration recipe pour adopting projects** : ajouter section
   dans le LOG.md P1-S3c expliquant comment migrer un v2-beta
   déployé :
   ```bash
   # 1. Pull docker-compose.yml, Dockerfile, install.sh updates
   cp -p ../devcontainer-tools-v2/templates/v2/docker-compose.yml .devcontainer/
   cp -p ../devcontainer-tools-v2/templates/v2/Dockerfile         .devcontainer/
   cp -p ../devcontainer-tools-v2/templates/v2/install.sh         .  # if root
   # 2. Ensure local files exist
   touch .devcontainer/firewall/domains.local.txt
   mkdir -p .devcontainer/firewall/policy.local.d
   # 3. Force base rebuild (Dockerfile.base sudoers changed)
   docker rmi claude-devcontainer-base:$(grep '^CLAUDE_CODE_VERSION=' .devcontainer/.env | cut -d= -f2)
   # 4. VS Code "Rebuild Container"
   ```

Validation (manual, end of session) :

```bash
# From inside container, as `node` :
# 1. Reads still work
cat /etc/devcontainer-firewall/domains.txt              # OK
cat /etc/devcontainer-firewall/domains.local.txt        # OK
cat /etc/devcontainer-firewall/dnsmasq.conf             # OK
cat /etc/devcontainer-firewall/policy.d/api.anthropic.com.yaml  # OK

# 2. Writes MUST fail
touch /etc/devcontainer-firewall/test-attack            # EROFS (image, ro)
echo "test" >> /etc/devcontainer-firewall/domains.local.txt  # EROFS

# 3. Workspace path writes also fail (no bind mount anymore)
ls /workspace/.devcontainer/firewall/                    # still visible via workspace
echo "test" >> /workspace/.devcontainer/firewall/domains.local.txt  # WRITES — but doesn't affect /etc anymore
# (workspace writes only modify the host file ; container's /etc/devcontainer-firewall/ is image-baked)

# 4. Sudo init-firewall.sh : removed from sudoers
sudo init-firewall.sh                                    # MUST fail "not in sudoers"

# 5. Firewall still works at boot
docker-compose restart                                   # init-firewall.sh runs as root via lifecycle
```

DoD at end of this session :
1. STATUS.md : ajouter row "3c — firewall-write-protection" entre
   les rows 3b et 4, flip 📋 → ✅, bump "Delivered" counter
   (3/6 → 4/7 si on insère 3c comme nouveau deliverable).
2. LOG.md : append `## P1-S3c — firewall-write-protection` section
   avec files touched, threat model, decision rationale (A vs B),
   migration recipe, validation output, gotchas.
3. KNOWLEDGE.md : nouvelle section "Security model" + amendement
   de "Image layer split" pour refléter le shift.
4. SCOPE.md : amender la section firewall data : préciser que tout
   est désormais baked, plus de bind mount.
5. ROADMAP.md : insérer row 3c dans la status table, mention dans
   "Part 1 — what's left", bump compteurs.
6. SECURITY.md créé dans templates/v2/ (et mirror dogfood).
7. Propose UN commit (DO NOT commit sans user confirmation) :
   ```
   Make firewall config immutable from container side

   - Remove the ./firewall:/etc/devcontainer-firewall:ro bind mount
     from docker-compose.yml. Firewall config now lives baked in
     the image only — runtime view is image-derived, not host-derived.
   - Project Dockerfile COPYs the full project-firewall set
     (domains.txt, domains.local.txt, policy.d/, policy.local.d/)
     instead of relying on the bind mount overlay.
   - install.sh ensures domains.local.txt + policy.local.d/ exist
     at install time so the project Dockerfile COPY never fails on
     missing source.
   - Drop the sudoers.d/node-firewall entry for init-firewall.sh
     (unused : the script is one-shot at boot, and the image is
     now immutable from the container). test-firewall.sh keeps
     its sudoers entry (read-only).
   - Net effect : Claude (node user) cannot tamper with firewall
     config. Edits require a rebuild — same security gate as any
     other image content. start/stop/start re-applies the baked
     config, never any in-container modification.
   - SECURITY.md created : formalised threat model + invariants
     + audit recipe. Flags remaining surfaces to investigate
     (.claude/, .claude-creds/, mitmproxy volume).
   ```
`````

## Next session

`part-1-session-4-fresh-install-test.md` — full Reopen-in-Container
sandbox validation. P1-S4 doit valider le post-S3c behavior (writes
EROFS, rebuild flow, install.sh handles fresh + reinstall).

## Open questions for future sessions

- **Audit complet des autres surfaces** : `.claude/` (writable volume),
  `.claude-creds/` (writable volume, OAuth tokens), mitmproxy volume,
  bash-history volume. À spec dans une session future "security-audit-2".
- **Lifecycle reload mechanism** : si on veut un "edit + reload sans
  rebuild" pour le user (pas Claude), pourrait implémenter un host-side
  watcher qui trigger un container restart sur modif `.devcontainer/firewall/*`.
  Hors scope ici, à designer si l'UX rebuild devient pénible en pratique.
