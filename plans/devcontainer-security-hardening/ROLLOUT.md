# Rollout — Devcontainer Security Hardening

> Entry point of this plan directory. For the actionable session table,
> see [STATUS.md](STATUS.md). For the reasoned journal of delivered
> sessions, see [LOG.md](LOG.md). For the technical inventory + threat
> surface, see [EXISTING.md](EXISTING.md).

## Goal — les 3 critères

**Objectif** : `node` user sandboxé dans le devcontainer.

Critères concrets :
1. `node` ne peut PAS relancer la machine seul
2. `node` ne peut PAS modifier le firewall sans rebuild
3. `node` ne peut PAS accéder à une ressource externe / exfiltrer sans rebuild

**Threat model** : `node` est UNTRUSTED (Claude potentiellement
prompt-injected, ou attaquant SSH-direct). Host est TRUSTED. Le rebuild
est un acte conscient host-side, accepté comme barrière.

**Hors scope (acceptés)** :
- Persistance node-level (modifier `~/.bashrc`, créer fichiers
  `~/.local/bin/...`, etc.). L'attaquant ne gagne pas plus que les
  droits qu'il a déjà.
- Modification de scripts bind-montés runnant en `node` (`shell-init.sh`,
  `sync-creds.sh`, etc.). Idem — pas d'escalation, pas d'exfil
  (le firewall les bloque tous).

## Sessions essentielles (3)

| # | Session | Rôle |
|---|---|---|
| 1 | `bake-firewall-config` | Bake TOUT ce qui est sécurité-critique (rules, mode, direct-tcp-allow) dans l'image. Drop le bind-mount. |
| 2 | `drop-env-injection` | Supprime la plomberie `/tmp/.firewall-env` (devenue obsolète après session 1). Privesc fix. |
| 6 | `adversarial-validation` | Red-team engagement — replay 13 vecteurs + hunt new ones. Gate du rollout. |

Total : ~5-6 h de travail sur 3 sessions.

## Sessions optionnelles (defense-in-depth, hors scope)

- Session 3 — `claude-hooks-allowlist` : protège contre la persistance
  via hooks Claude. Hook tourne en node = pas d'exfil possible si
  firewall locked (sessions 1+2). Out of scope per les 3 critères.
- Session 4 — `firewall-mode-baked-only` : MERGÉ dans session 1.
- Session 5 — `mitm-log-restrict` : protège contre la lecture locale
  des bearer tokens dans logs. Lecture locale ≠ exfil. Out of scope per
  les 3 critères.

Ces sessions restent comme **référence** pour un v2 si la posture doit
être renforcée davantage.

## Navigation

| File | When to open |
|---|---|
| **[STATUS.md](STATUS.md)** | "Where are we, what's next ?" — actionable session table |
| **[LOG.md](LOG.md)** | "What was done, why, what gotchas ?" — append-only journal |
| **[EXISTING.md](EXISTING.md)** | "What does the code look like today ?" — factual inventory + threat surface |
| sessions/session-NN-*.md | Prompt to paste into a new Claude chat to start session NN |

## How to use

1. **To resume work** : open [STATUS.md](STATUS.md), find the next 📋
   session, click `→ prompt` and paste into a fresh Claude Code session.
2. **To check what was done before** : read [LOG.md](LOG.md).
3. **To understand current code state + threat surface** : read
   [EXISTING.md](EXISTING.md).

## Decisions log

- **2026-05-22 — Bake-only over `:ro` overlay** : option B retenue pour
  session 1. Justification : `:ro` overlay laisse les fichiers writable
  via workspace mount. Bake-only = vraie immutabilité runtime.

- **2026-05-22 — Audit report committé** dans
  `.devcontainer/SECURITY-AUDIT-2026-05.md`. Vit avec le code, référencé
  par les commits des sessions.

- **2026-05-22 — Threat model affiné aux 3 critères** : node sandboxé
  signifie spécifiquement (1) pas de relance autonome, (2) pas de modif
  firewall sans rebuild, (3) pas d'exfil sans rebuild. Tout vecteur
  hors de ces critères = defense-in-depth optionnel, pas critique.

- **2026-05-22 — Sessions 3 et 5 marquées defense-in-depth** :
  hooks Claude et mitm logs sont des vecteurs node-level (pas
  d'élévation, pas d'exfil après sessions 1+2). Out of critical scope.

- **2026-05-22 — Session 4 mergée dans session 1** : le bake du fichier
  `default-mode` + `direct-tcp-allow.txt` est de la même nature que le
  bake des rules → cohérent dans une seule session.

- **2026-05-22 — `.env` reste workspace-éditable** pour les vars non
  sécurité-critiques (TZ, version pins, debug toggle). Seules les vars
  qui contrôlent le firewall sont migrées vers des fichiers baked dans
  `firewall/`.

- **2026-05-22 — Renommer `allowed-hosts.txt` → `direct-tcp-allow.txt`**
  pour expliciter la sémantique (bypass mitmproxy direct TCP, pas un
  allowlist HTTP).

## Reuse from prior planning

La session 1 reprend matériellement la spec existante
[part-1-session-3c-firewall-write-protection.md](../devcontainer-tools-v2-migration/sessions/part-1-session-3c-firewall-write-protection.md)
du rollout v2-migration. Cette spec couvre déjà l'option B (bake-only)
en détail. Session 1 ici étend le scope avec mode + direct-tcp-allow +
commit du rapport d'audit en préambule.
