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
networks: all            # ou une liste : [bluesky, mastodon, threads, linkedin, facebook, instagram, discord]
image: img/sandbox-dns.png
alt: Terminal showing dnsmasq answering REFUSED to an unlisted domain
tags: [ClaudeCode, devcontainer, infosec]
link: https://github.com/meitogi/devcontainer-sandbox
status: draft
---
Outbound DNS in Dev Sandbox answers REFUSED for anything not on the
allowlist. Not NXDOMAIN: REFUSED. The agent learns in one query that the
network is not negotiable.

## long
(variante longue pour LinkedIn et Facebook, sans lien dans le corps ; le
lien va en premier commentaire)
```

Règles appliquées par la page et par le script : Bluesky 300 caractères,
lien transformé en facet, `tags` ignorés ; Mastodon et Threads 500
caractères, tags ajoutés en fin ; LinkedIn et Facebook prennent la
variante `## long` sinon le texte principal ; Instagram saute le post
s'il n'y a pas d'`image` ; `networks: all` vaut tous les réseaux
automatisés ; `status` passe `draft` → `ready` → `published`, avec le
détail par réseau noté dans le fichier à la publication.

## Quatre flux

1. **Claude écrit** (devcontainer) : le prompt génère N posts en `draft`,
   leurs cartes, et commite. Le push reste à l'utilisateur (la porte).
2. **La page relit** (navigateur, Pages ou local) : liste `posts/` via
   l'API contents de GitHub, édite, prévisualise par réseau, passe en
   `ready` avec une date (commit via la même API), ou **publie tout de
   suite** sur Bluesky, Mastodon, Discord en direct.
3. **L'Action publie** : chaque heure, `publish.mjs` prend les `ready`
   échus, poste sur Bluesky, Mastodon, Threads, Facebook, Instagram,
   LinkedIn, Discord et Dev.to, déplace dans `published/`, commite. Un
   réseau en échec n'arrête pas les autres : il est noté dans le fichier
   et retenté à l'heure suivante.
4. **Le manuel pré-rempli** : X, LinkedIn, Reddit, HN, Threads. Un bouton
   par réseau copie l'image dans le presse-papiers puis ouvre le composeur
   du réseau déjà rempli par son URL d'intent (section suivante). Il reste
   un Ctrl+V et un clic « publier ».

| Réseau | Depuis la page | Depuis l'Action / CLI | Manuel |
|---|---|---|---|
| Bluesky | direct | oui | — |
| Mastodon | direct | oui | — |
| Discord | direct | oui | — |
| Dev.to | via dispatch de l'Action (pas de CORS en POST) | oui | — |
| X | intent pré-rempli | — | Ctrl+V l'image, publier ; lien en réponse |
| Reddit, HN | intent pré-rempli (titre, texte ou URL) | — | publier |
| Threads | intent en secours | oui, image par URL raw.githubusercontent.com | — |
| Product Hunt | copier | — | formulaire |
| Facebook, page | intent en secours | oui, API Graph, même app Meta | — |
| Instagram, compte pro | — | oui, API Graph, image par URL raw.githubusercontent.com ; saute sans image | — |
| LinkedIn | intent en secours | oui, API Posts, reconsentement tous les 60 jours | — |
| YouTube Shorts, TikTok | télécharger le MP4 + texte copié | — | upload depuis Studio ou l'app |

## L'app Meta et LinkedIn : jetons et rafraîchissement

- **Une app Meta** (developers.facebook.com, gratuite) porte Threads,
  la page Facebook et le compte Instagram pro. Ton propre compte y est
  ajouté comme testeur : aucune revue d'app, aucune vérification
  d'entreprise tant que seul ton compte publie.
- **Threads** : jeton long de 60 jours, rafraîchi par un workflow mensuel
  `refresh-tokens.yml` qui réécrit le secret avec `gh secret set` (un
  PAT `GH_PAT` à portée Secrets sur ce seul repo).
- **Facebook et Instagram** : le jeton de page dérivé d'un jeton
  utilisateur long n'expire pas. Instagram publie en deux appels
  (`/media` avec `image_url`, puis `/media_publish`) ; l'image doit être
  publique, d'où le commit dans `posts/img/` et l'URL raw.
- **LinkedIn** : app rattachée à une page entreprise Meitogi, produit
  « Share on LinkedIn » ; jeton de 60 jours sans rafraîchissement
  automatique pour une app self-serve. `npm run linkedin:auth` ouvre
  l'URL de consentement, prend le code, échange et pose le secret : deux
  minutes tous les deux mois, rappelées par l'Action quand il reste dix
  jours.
- Limites connues : Threads 250 posts / 24 h, Instagram de l'ordre de 50
  / 24 h, LinkedIn 150 / jour. Sans objet à deux posts par semaine.

## Boîte de réception et réponses

Répondre est la moitié du temps hebdomadaire ; la page l'absorbe pour les
deux réseaux qui exposent leurs notifications gratuitement et avec CORS :

- **Bluesky** : `app.bsky.notification.listNotifications` (mentions,
  réponses, citations, nouveaux abonnés) ; une réponse est un
  `createRecord` avec `reply.root` et `reply.parent`.
- **Mastodon** : `GET /api/v1/notifications` ; une réponse est un statut
  avec `in_reply_to_id`.
- **Les autres** (X, LinkedIn, Threads, Reddit, HN) n'ont pas de lecture
  gratuite : un bouton par réseau ouvre sa page de notifications.

Un onglet « Inbox » liste tout ça par ordre inverse, avec une zone de
réponse sous chaque entrée et un marquage « traité » stocké en
`localStorage`. Pas de notification push : l'onglet se consulte pendant
la session hebdo.

## Pré-remplissage des réseaux manuels

Pas d'extension navigateur : les intents suffisent, et une extension qui
pilote le composeur de X se casse à chaque refonte du DOM. La page
construit l'URL, l'ouvre dans un nouvel onglet, et a mis l'image dans le
presse-papiers juste avant (`navigator.clipboard.write` avec le PNG ;
tous ces composeurs acceptent une image collée).

| Réseau | URL d'intent | Pré-remplit |
|---|---|---|
| X | `https://x.com/intent/post?text=…&url=…` | texte, lien |
| Bluesky | `https://bsky.app/intent/compose?text=…` | texte (secours si la publication directe échoue) |
| Mastodon | `https://<instance>/share?text=…` | texte (idem) |
| Threads | `https://www.threads.net/intent/post?text=…` | texte, à vérifier au build |
| LinkedIn | `https://www.linkedin.com/feed/?shareActive=true&text=…` | texte, à vérifier au build |
| Reddit | `https://www.reddit.com/r/<sub>/submit?title=…&selftext=true&text=…` | titre, texte |
| Hacker News | `https://news.ycombinator.com/submitlink?u=…&t=…` | URL, titre |
| Facebook | `https://www.facebook.com/sharer/sharer.php?u=…` | lien seulement ; le texte se colle, à vérifier au build |
| TikTok | aucun intent web | vidéo enregistrée, publication depuis l'app |

Aucun intent n'attache d'image : d'où le presse-papiers. Le post `.md`
peut porter un `title` (Reddit, HN) et un `sub` (Reddit) en frontmatter.

Mécanique de la fenêtre, pour travailler côte à côte :

- **Pas d'iframe** : tous ces réseaux envoient `frame-ancestors 'none'`
  ou `X-Frame-Options: DENY`, le composeur refuse de s'afficher dans une
  page d'une autre origine.
- **Popup calée à droite** : `window.open(url, 'composer', 'popup,
  width=W,height=H,left=screen.availWidth-W,top=0')`, même nom de
  fenêtre pour tous les réseaux, donc une seule popup réutilisée.
- **Bouton « Docker »** : une page ne peut pas se redimensionner
  elle-même, seule une fenêtre ouverte par script le peut. Le bouton
  rouvre la page dans une popup calée à gauche et ferme l'onglet
  d'origine ; largeurs mémorisées dans `localStorage`. Sur téléphone, le
  bouton n'apparaît pas.
- **Glisser-déposer de la vignette** vers le composeur : Chrome le
  permet si `dragstart` pose `DownloadURL` (`image/png:<nom>:<url>`), le
  composeur reçoit alors un fichier. Firefox et Safari reçoivent un lien,
  inutile. Le presse-papiers reste la voie universelle, l'image y est
  copiée juste avant l'ouverture du composeur.

Si un jour le Ctrl+V pèse, une **extension WebExtension minimale** (un
content script par réseau qui injecte l'image dans le composeur) est
l'étape suivante ; elle reste hors périmètre tant que l'intent fait le
travail.

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

## Vidéo : capturer brut, monter par script

Zéro compétence montage requise. On capture brut, un script fait le reste,
et Claude écrit les textes.

**Capturer** (toujours brut : 16:9, 30 i/s, 20 à 40 s, sans rien couper) :

- Terminal et wizard Dev Sandbox : **pas de capture à la main**. VHS
  (charmbracelet/vhs, open source) rend un GIF ou un MP4 depuis un
  script `.tape` que Claude écrit : les frappes, les pauses, la taille
  de fenêtre. Reproductible, refait à chaque release. VHS télécharge un
  Chromium ; si le firewall le bloque, repli sur asciinema + agg
  (enregistrement réel puis GIF, deux binaires cargo, pas de navigateur).
- CTR dans le navigateur : le navigateur agent enregistre lui-même
  (skill `agent-browser`), Claude lance la course et sort le MP4.
- PurrPause sur Windows : Xbox Game Bar, `Win+G` puis `Win+Alt+R`,
  intégré à Windows, gratuit. Sur macOS, `Cmd+Shift+5`. Sur téléphone,
  l'enregistreur d'écran du système.

**Monter** : `scripts/video.mjs`, des recettes ffmpeg sans interface,
paramétrées par un petit fichier à côté de la capture :

```
clip: raw/ctr-race.mp4
in: 00:04        out: 00:16        speed: 1.0
title: "CTR in your browser"       # carte satori 1,5 s en ouverture
captions:                          # incrustées, lisibles sans le son
  00:00 Drop your own disc
  00:05 Online race, WebRTC
outputs: [landscape, portrait, gif]
```

Recettes : couper, accélérer, carte titre, sous-titres incrustés depuis
un `.srt` que Claude écrit à partir du post, format portrait 9:16 par
recadrage centré ou fond flou, GIF à palette optimisée sous le million
d'octets de Bluesky, MP4 H.264 sans piste audio (les réseaux se
regardent coupés) ou avec une piste MP3 si une voix est ajoutée un jour
(Piper, synthèse vocale locale et libre, en option). Tous les rendus
s'ouvrent dans l'aperçu VS Code.

**Dans la page, en local** (session 8) : un panneau « Clip » lit la
capture brute, un curseur pose le point d'entrée et de sortie, un champ
par sous-titre, un clic « Rendre » appelle `video.mjs` via le serveur
local. Pas de timeline, pas de pistes : c'est volontaire.

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
| `graph.threads.net`, `graph.facebook.com`, `graph.instagram.com`, `api.linkedin.com`, `www.linkedin.com` | phase 3, optionnel | idem |
| `docs.bsky.app`, `atproto.com`, `docs.joinmastodon.org`, `developers.forem.com`, `docs.discord.com`, `developers.facebook.com` | docs pour Claude pendant le build | idem |

Volontairement absents : `api.x.com` (payant), `gql.hashnode.com`
(payant), `oauth.reddit.com` (approbation manuelle), `youtube.googleapis.com`
(vidéos forcées en privé). Rien à ajouter dans le devcontainer lui-même.

## Sessions de construction (à scaffolder avec `/prepare-plan`)

1. **socle** : repo, `index.html` réglages + liste + éditeur + prévisualisation par réseau, format des posts, `brand.mjs`.
2. **publication directe** : Bluesky (facets, blob), Mastodon, Discord depuis la page ; `publish.mjs` en CLI, `.env`.
3. **planification et intents** : `publish.yml` cron + dispatch, Pages, Dev.to, les boutons pré-remplis et le presse-papiers.
4. **Meta et LinkedIn** : Threads, page Facebook, Instagram, LinkedIn dans `publish.mjs` ; `refresh-tokens.yml` ; `linkedin:auth`.
5. **images et rédaction** : `card.mjs`, memegen dans la page, le prompt de posts transformé en skill locale, premier lot de 6 posts.
6. **bilan** : `review.mjs`, `metrics.md`, et la revue du premier mois.
7. **boîte de réception et réponses** : notifications Bluesky et Mastodon dans la page, réponse en un clic, liens directs vers les autres réseaux.
8. **clip** : `video.mjs` (recettes ffmpeg, sous-titres, formats), VHS ou asciinema + agg pour le terminal, le panneau « Clip » du serveur local.
