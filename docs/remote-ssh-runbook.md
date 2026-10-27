# Passer au deuxième Mac — runbook

Le jour où le Mac 2 est branché : les commandes, dans l'ordre, pour ce
workspace (devcontainer-tools-v2), **ragnarokonline** et **symptems**. Le
pourquoi de chaque étape, et les pièges, sont dans le guide public :
[develop-from-another-mac.md](https://github.com/meitogi/devcontainer-sandbox/blob/master/docs/how-to/develop-from-another-mac.md)
(dans ce workspace : `packages/devcontainer-sandbox/docs/how-to/`, à côté du script).

- **Mac 1** : ce MacBook — la fenêtre VS Code, Fork, les identifiants GitHub.
- **Mac 2** : le nouveau Mac — les fichiers des projets, Docker, les containers.

Les commandes Mac 1 se lancent dans le **Terminal macOS**, à deux endroits :

- **depuis ce workspace** : ce workspace (`.`) et ragnarokonline, son voisin
  (`../ragnarokonline`) ;
- **depuis le dossier de symptems**, qui est ailleurs : tape `cd ` (avec
  l'espace), glisse le dossier symptems du Finder dans le Terminal, Entrée.
  Sans argument, le script travaille sur le dossier où il est lancé.

Une fois, depuis ce workspace, copie le script dans ton home pour l'avoir
partout :

```
cp packages/devcontainer-sandbox/docs/how-to/remote-sync.sh ~/remote-sync.sh
```

---

## 1. Préparer le Mac 2 (sur le Mac 2)

Guide public, étape 1 : Remote Login activé, Docker Desktop lancé, Node 18+
(nvm ou Homebrew), puis :

```
xcode-select --install
```
```
scutil --get LocalHostName
```

Note le nom affiché.

## 2. Joindre le Mac 2 (sur le Mac 1)

Ajoute à `~/.ssh/config` l'alias `mac2` (guide public, étape 2 : `HostName
<le nom>.local`, `User <ton user sur le Mac 2>`), puis :

```
ssh-copy-id mac2
```
```
ssh mac2 git --version
```

La deuxième doit afficher une version. Dans VS Code : extension **Remote - SSH**.

## 3. Déménager les trois projets (sur le Mac 1)

**Ferme d'abord les fenêtres VS Code des trois projets**, celle-ci comprise
(la conversation Claude en cours est dans un volume : elle partira avec lui).

Copie des dossiers, au même chemin absolu sur le Mac 2 :

Depuis ce workspace :

```
bash ~/remote-sync.sh seed mac2 . ../ragnarokonline
```

Depuis le dossier de symptems :

```
bash ~/remote-sync.sh seed mac2
```

Copie des volumes Docker — le script pose une question par volume. Réponds
d'après le **point de montage** affiché :

| monté sur | contenu | copier ? |
|---|---|---|
| `/home/node/.claude` | historique et sessions Claude | **oui** |
| `/home/node/.claude-creds` | login Claude | **oui** |
| `/commandhistory` | historique du shell | oui |
| `/var/lib/mitmproxy` | CA du firewall | oui (petit) |
| `/var/lib/mysql` (ragnarokonline) | base rAthena | **oui** |
| `/var/lib/docker` (dind) | images du Docker imbriqué | **non** — des dizaines de Go, se re-télécharge |
| tout autre cache | — | non |

Depuis ce workspace :

```
bash ~/remote-sync.sh volumes mac2 . ../ragnarokonline
```

Depuis le dossier de symptems :

```
bash ~/remote-sync.sh volumes mac2
```

Un volume partagé entre projets (le login Claude, si `CLAUDE_CREDS_VOLUME` est
commun) n'est copié qu'une fois : les fois suivantes il « holds data » et il
est gardé.

Puis brancher chaque repo sur le Mac 2 (ajoute la remote `mac2` partout) :

Depuis ce workspace :

```
bash ~/remote-sync.sh pull mac2 . ../ragnarokonline
```

Depuis le dossier de symptems :

```
bash ~/remote-sync.sh pull mac2
```

## 4. Ouvrir un projet sur le Mac 2 (sur le Mac 1)

Avec la commande `code` installée (VS Code : *Shell Command: Install 'code'
command in PATH*), depuis ce workspace :

```
code --remote ssh-remote+mac2 "$PWD"
```
```
code --remote ssh-remote+mac2 "$(cd ../ragnarokonline && pwd -P)"
```
Et depuis le dossier de symptems :

```
code --remote ssh-remote+mac2 "$PWD"
```

Puis **Dev Containers : Reopen in Container**. Sans `code` : Remote-SSH :
Connect to Host → mac2 → Open Folder, même chemin.

## 5. Les ports, projet par projet

Ce qui est transféré arrive sur le `localhost` **du Mac 1**, par ssh, en TCP
seulement.

- **devcontainer-tools-v2** — aucun port forwardé automatiquement
  (`otherPortsAttributes: ignore`), comme aujourd'hui : « Forward Port » à la
  main dans le panneau Ports. Attention : le chemin WebRTC direct
  (`127.0.0.1:50100:50101/udp`) est en **UDP** et lié au loopback **du Mac 2** :
  il ne marche pas depuis le Mac 1. Il ne marche que depuis un navigateur sur
  l'écran du Mac 2. Le rendre joignable demande de changer le compose : ligne
  de suivi dans le plan remote-ssh-anywhere.
- **ragnarokonline** — les 13 `forwardPorts` (7306, 7310-7312, 7316-7318,
  7321, 7331, 7335, 7336, 7340, 7341) et les ports nommés (6980 static dist,
  6988 robrowser prod, 7300 robrowser dev, 7302/7303 cyro, 7305 gfx asm)
  arrivent sur le Mac 1. Les `<name>.robrowser.localhost` marchent (le
  `.localhost` résout sur le Mac 1). Beaucoup sont en `requireLocalPort` :
  **ne relance pas ragnarokonline en local sur le Mac 1 en même temps**, sinon
  les ports sont déjà pris. Le Chromium de l'agent browser est celui **du
  Mac 2** (LaunchAgent à installer là-bas, ou le sidecar headless).
- **symptems** — le control plane d'orchestration **55280** et les slots
  **55300-55307** sont en auto-forward (`remote.autoForwardPortsSource:
  process`) : **oui, le forward les amène sur le Mac 1**, et
  `s<k>.symptems.localhost` marche. Ils ne sont pas joignables depuis une autre
  machine : il faudrait un `ports:` publié hors loopback dans le compose.
  Le **sandbox des agents** (Postgres 55420, SeaweedFS 55421, Redis 55422,
  `docker/compose/agent-sandbox.yml`) n'est pas dans le devcontainer : il se
  lance côté hôte (`wtf claude sandbox init`) et le container le joint par
  `host.docker.internal`, c'est-à-dire **le Mac 2**. Lance-le donc sur le Mac 2,
  dans son Terminal, comme tu le fais ici aujourd'hui (il faut `wtf` installé
  côté hôte là-bas). Il est jetable : rien à copier, `volumes` ne le liste pas.
  Il publie sur toutes les interfaces : sur le Mac 2 aussi, il sera joignable
  depuis le réseau local.

Notifications : elles s'afficheront sur l'écran **du Mac 2** tant que le daemon
n'est pas remplacé par l'extension Notify.

## 6. Au quotidien (sur le Mac 1)

Ramener les commits de Claude, puis relire et pousser dans Fork :

Depuis ce workspace :

```
bash ~/remote-sync.sh pull mac2 . ../ragnarokonline
```

Depuis le dossier de symptems :

```
bash ~/remote-sync.sh pull mac2
```

Après un amend ou un drop dans Fork, une fois poussé :

Depuis ce workspace :

```
bash ~/remote-sync.sh push-back mac2 . ../ragnarokonline
```

Depuis le dossier de symptems :

```
bash ~/remote-sync.sh push-back mac2
```

Il affiche, par branche, les commits que le Mac 2 perdrait et la commande
`git reset --keep mac1/<branche>` à lancer **dans le container** (ou à demander
à Claude là-bas).

## 7. Si le remote casse : retour en local (sur le Mac 1)

Depuis ce workspace :

```
bash ~/remote-sync.sh pull mac2 --wip . ../ragnarokonline
```

Depuis le dossier de symptems :

```
bash ~/remote-sync.sh pull mac2 --wip
```

Les commits reviennent, le travail non commité aussi (`git stash apply
mac2/wip` dans chaque repo qui en a un), et les fichiers non suivis sont listés
par nom. Puis ouvre le projet en local sur le Mac 1, **Reopen in Container**.
Les volumes du Mac 1 sont toujours là : ne les supprime pas (`down -v`) tant que
le Mac 2 n'a pas fait ses preuves.

## À savoir

- Les worktrees Claude faits dans un container (315 sous ragnarokonline, 9 ici)
  ne s'ouvrent pas depuis un Mac : le script les saute. `seed` les copie quand
  même, avec le reste du dossier.
- 3 repos de ragnarokonline utilisent Git LFS : un `pull` ramène les
  pointeurs, pas les fichiers, des commits pas encore sur GitHub.
- Les stashes (12 dans symptems) ne voyagent pas avec `pull` : après `seed`,
  ils sont présents des deux côtés. Les nouveaux restent là où ils sont créés.
