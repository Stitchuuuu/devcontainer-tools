# L'outil — une page, un repo, zéro serveur

## Principe

Quatre rôles, aucun daemon :

| Rôle | Tenu par | Pourquoi |
|---|---|---|
| File d'attente | le repo `meitogi/megaphone`, un `.md` par post | versionné, relisible dans Fork, éditable par Claude et par la page |
| Interface | `index.html`, une page statique sur GitHub Pages **et** en local | CORS vérifié : Bluesky, Mastodon, Discord et api.github.com acceptent un appel direct depuis le navigateur |
| Planificateur | GitHub Actions, cron horaire + `workflow_dispatch` | gratuit et illimité sur un repo public ; les commits de l'Action maintiennent le repo actif (le cron s'éteint après 60 jours sans activité) |
| Rédacteur | Claude Code dans le devcontainer, sur l'abonnement Max | [prompt-posts.md](prompt-posts.md) ; aucune clé API |

Le serveur maison n'est pas nécessaire. Il le deviendrait seulement pour
auto-héberger memegen sans filigrane (image Docker officielle) : option,
pas prérequis.

## Arborescence

```
apps/megaphone/                  repo public meitogi/megaphone
  index.html                     la page (HTML + JS vanilla, un seul fichier)
  posts/                         la file : 2026-11-03-sandbox-dns-refused.md
  posts/img/                     PNG des cartes, memes, captures
  published/                     déplacé ici par l'Action après publication
  ideas.md                       backlog, une ligne par idée
  metrics.md                     une ligne par mois
  scripts/
    card.mjs                     titre → PNG 1200×630 (satori + resvg-wasm)
    brand.mjs                    avatar et bannière depuis les SVG
    publish.mjs                  publie un post ou tous les posts échus
    review.mjs                   bilan mensuel → metrics.md
  .github/workflows/
    publish.yml                  cron */60 + dispatch → scripts/publish.mjs
    pages.yml                    déploie index.html
```

## Format d'un post

```
---
date: 2026-11-03T09:00+01:00
networks: [bluesky, mastodon, linkedin]
image: img/sandbox-dns.png
alt: Terminal showing dnsmasq answering REFUSED to an unlisted domain
tags: [ClaudeCode, devcontainer, infosec]
link: https://github.com/meitogi/devcontainer-sandbox
status: draft
---
Outbound DNS in Dev Sandbox answers REFUSED for anything not on the
allowlist. Not NXDOMAIN: REFUSED. The agent learns in one query that the
network is not negotiable.

## linkedin
(variante longue, sans lien dans le corps ; le lien va en premier commentaire)
```

Règles appliquées par la page et par le script : Bluesky 300 caractères,
lien transformé en facet, `tags` ignorés ; Mastodon 500 caractères, tags
ajoutés en fin ; LinkedIn prend la variante `## linkedin` sinon le texte
principal ; `status` passe `draft` → `ready` → `published`.

## Quatre flux

1. **Claude écrit** (devcontainer) : le prompt génère N posts en `draft`,
   leurs cartes, et commite. Le push reste à l'utilisateur (la porte).
2. **La page relit** (navigateur, Pages ou local) : liste `posts/` via
   l'API contents de GitHub, édite, prévisualise par réseau, passe en
   `ready` avec une date (commit via la même API), ou **publie tout de
   suite** sur Bluesky, Mastodon, Discord en direct.
3. **L'Action publie** : chaque heure, `publish.mjs` prend les `ready`
   échus, poste, déplace dans `published/`, commite. Dev.to idem.
4. **Le manuel assisté** : LinkedIn, Reddit, HN, Product Hunt. La page
   a un bouton « copier » par réseau et ouvre l'image à joindre.

| Réseau | Depuis la page | Depuis l'Action / CLI | Manuel |
|---|---|---|---|
| Bluesky | direct | oui | — |
| Mastodon | direct | oui | — |
| Discord | direct | oui | — |
| Dev.to | via dispatch de l'Action (pas de CORS en POST) | oui | — |
| LinkedIn | copier | — | coller, image, lien en commentaire |
| Reddit, HN, Product Hunt | copier | — | coller |
| Threads | phase 3 : via l'Action, image par URL raw.githubusercontent.com | — | — |

## En local et en ligne de commande

```
cd apps/megaphone
npm run ui                       index.html sur localhost:4173, port forwardé par VS Code
npm run card -- "Titre" img/x.png
npm run publish -- posts/2026-11-03-sandbox-dns-refused.md
npm run due                      ce que fait l'Action
npm run review                   une ligne dans metrics.md
gh workflow run publish.yml      déclenche l'Action à distance
```

La page en local lit le même `localStorage` que la version Pages si elle
est ouverte dans le même navigateur, sinon on ressaisit les réglages.

## Images, toutes gratuites et sans filigrane sauf mention

- **Cartes** : satori (HTML/CSS → SVG) + `@resvg/resvg-wasm` (SVG → PNG).
  Pur JS et WASM, pas de binaire natif : compatible avec le bind mount.
- **Memes** : `api.memegen.link/images/<template>/<haut>/<bas>.png`,
  prévisualisé en `<img>` dans la page, téléchargé par le script au moment
  de poster. Petit filigrane ; auto-hébergeable sur le serveur maison si
  ça gêne.
- **Captures de code** : silicon (`cargo install silicon`, optionnel) ou
  une capture VS Code.
- **GIF** : enregistrement d'écran puis ffmpeg (déjà dans l'image) ;
  pour un Short, le même fichier en MP4 vertical.

## Dépendances npm de l'app (versions à relever au moment de l'installation)

`@atproto/api` (session, blob, facets pour les liens), `satori`,
`@resvg/resvg-wasm`. Mastodon, Discord, Dev.to et GitHub en `fetch` nu.
Frontmatter parsé par une fonction de dix lignes, pas de dépendance.

## Sécurité

- Le PAT est **fine-grained, limité au repo `megaphone`**, Contents +
  Actions. Il ne vit que dans `localStorage` et dans `.env` gitignoré.
- Les secrets de publication vivent dans les secrets Actions ; un fork ne
  les voit pas et `workflow_dispatch` n'est ouvert qu'aux collaborateurs.
- Le repo est public : les brouillons le sont aussi. Rien de confidentiel
  dans un post, et aucune clé dans un `.md`.

## Analyse des domaines

| Hôte | Usage | Où |
|---|---|---|
| `bsky.social` | session, blob, createRecord | `domains.d/social.txt` + local |
| `public.api.bsky.app` | lectures (abonnés, bilan) | idem |
| `fosstodon.org`, `hachyderm.io`, `mastodon.social` | statuts, médias ; à élaguer à l'instance choisie | idem |
| `dev.to` | articles | déjà dans `domains.local.txt` |
| `discord.com` | webhook | déjà dans `domains.local.txt` |
| `api.github.com`, `github.com`, `raw.githubusercontent.com` | contents, dispatch, traffic, images publiques | baseline |
| `api.memegen.link`, `memegen.link` | memes | `domains.d/social.txt` + local |
| `graph.threads.net`, `api.linkedin.com`, `www.linkedin.com` | phase 3, optionnel | idem |
| `docs.bsky.app`, `atproto.com`, `docs.joinmastodon.org`, `developers.forem.com`, `docs.discord.com`, `developers.facebook.com` | docs pour Claude pendant le build | idem |

Volontairement absents : `api.x.com` (payant), `gql.hashnode.com`
(payant), `oauth.reddit.com` (approbation manuelle), `youtube.googleapis.com`
(vidéos forcées en privé). Rien à ajouter dans le devcontainer lui-même.

## Sessions de construction (à scaffolder avec `/prepare-plan`)

1. **socle** : repo, `index.html` réglages + liste + éditeur + prévisualisation, format des posts, `brand.mjs`.
2. **publication directe** : Bluesky (facets, blob), Mastodon, Discord depuis la page ; `publish.mjs` en CLI, `.env`.
3. **planification** : `publish.yml` cron + dispatch, Pages, Dev.to, bouton « copier » LinkedIn et consorts.
4. **images et rédaction** : `card.mjs`, memegen dans la page, le prompt de posts transformé en skill locale, premier lot de 6 posts.
5. **bilan** : `review.mjs`, `metrics.md`, et la revue du premier mois.
