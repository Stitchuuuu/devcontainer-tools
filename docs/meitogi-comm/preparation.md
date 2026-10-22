# Préparation — comptes, paramétrage, soumissions

Tout ce qui se fait **à la main, côté host, une seule fois**. Dans l'ordre.
Rien ici ne coûte un centime.

## A. Identité (avant tout compte)

- [ ] **Avatar** : `meitogi-avatar.svg` (croissant argent sur acier) rendu
  en PNG 400×400 et 1024×1024. **Bannière** 1500×500 : le motif 石 de
  `meitogi-icon.svg` sur fond crème, texte « meitogi · open-source tooling
  for AI coding agents ». Un script `scripts/brand.mjs` les générera dans
  la session 1 de l'outil (resvg-wasm, pas de binaire natif).
- [ ] **Bio commune** (proposition, à ajuster) :
  > Open-source tooling for working seriously with AI coding agents.
  > Dev Sandbox (a devcontainer Claude Code cannot leave), CTR in your
  > browser, a dancing-cat break reminder. Building in public from France.
  > github.com/meitogi
- [ ] **Profil README** `meitogi/meitogi` : les trois produits avec un GIF
  chacun, lien croisé vers Stitchuuuu, badges npm / ghcr. Déjà prévu dans
  la candidature Claude for OSS.
- [ ] **Nom de domaine : aucun à acheter.** Il n'existe pas de `meitogi.*`
  et on n'en a pas besoin : Pages sur `meitogi.github.io`, CTR sur
  `ctr.suuuper.app`. Option gratuite à vérifier au moment : un site
  utilisateur `meitogi/meitogi.github.io` permet d'utiliser
  `@meitogi.github.io` comme handle Bluesky via le fichier
  `.well-known/atproto-did` (pratique courante, non vérifiée ici).

## B. Comptes à créer

| Réseau | À faire | Ce qu'il faut récupérer |
|---|---|---|
| Bluesky | compte `meitogi.bsky.social` ; Settings → Privacy and security → App passwords → nouveau | handle + **app password** |
| Mastodon | choisir l'instance : fosstodon.org (OSS, vérifier si sur invitation), sinon hachyderm.io, sinon mastodon.social ; Preferences → Development → New application, scopes `write:statuses write:media read:accounts` | URL de l'instance + **access token** |
| Dev.to | compte ; Settings → Extensions → DEV Community API Keys | **api key** |
| GitHub | repo public `meitogi/megaphone` (public : Pages gratuit et minutes Actions illimitées sont réservés aux repos publics) ; Settings → Pages → source « GitHub Actions » ; Settings → Secrets and variables → Actions : les secrets du tableau C | un **fine-grained PAT** limité à ce repo : Contents read/write, Actions read/write, expiration 1 an |
| Discord | optionnel : un salon « announcements » sur un serveur à toi → Integrations → Webhooks | **webhook URL** |
| Hacker News | compte, dès maintenant : un compte ancien passe mieux ; commenter un peu avant le premier Show HN | — |
| Reddit | compte ; répondre à des questions sur r/ClaudeAI pour atteindre 50 karma avant tout post vitrine | — |
| Product Hunt | compte dès maintenant, suivre quelques lancements | — |
| Lobste.rs | demander une invitation sur le chat ; 70 jours sans auto-promo ensuite | — |
| X, Threads, YouTube | réserver le handle `meitogi`, avatar, bio, lien GitHub, rien d'autre | — |

## C. Paramétrage

### Secrets du repo `megaphone` (Settings → Secrets → Actions)

| Secret | Valeur | Utilisé par |
|---|---|---|
| `BLUESKY_HANDLE` | `meitogi.bsky.social` | publish.yml |
| `BLUESKY_APP_PASSWORD` | l'app password | publish.yml |
| `MASTODON_URL` | `https://fosstodon.org` | publish.yml |
| `MASTODON_TOKEN` | l'access token | publish.yml |
| `DEVTO_API_KEY` | optionnel | publish.yml |
| `DISCORD_WEBHOOK` | optionnel | publish.yml |

### La page (une fois, par navigateur)

Le panneau « Réglages » de la page stocke dans `localStorage` : le PAT
GitHub, le handle + app password Bluesky, l'URL + token Mastodon, le
webhook Discord. Rien ne quitte le navigateur sauf vers ces API. À refaire
sur chaque navigateur (desktop, téléphone).

### Le devcontainer

- `apps/megaphone/.env` (gitignoré) avec les mêmes six valeurs pour la
  route ligne de commande.
- Firewall : déjà fait. `domains.d/social.txt` (committé, pris au prochain
  rebuild) et son miroir dans `domains.local.txt`. Pour l'activer sans
  rebuild, depuis un terminal **host** dans `.devcontainer/` :

  ```
  wtf firewall reload
  ```

  puis supprimer le bloc miroir de `domains.local.txt` après le prochain
  rebuild. Rien d'autre à installer : ffmpeg, gh et node 24 sont dans
  l'image ; satori et resvg-wasm sont des dépendances npm de l'app.

## D. Soumissions one-shot (après 6 posts publiés et un README propre)

- [ ] awesome-claude-code (hesreallyhim) : formulaire d'issue web, une
  ressource par issue, description factuelle sans emoji, catégorie
  « Security » ou « Providers, Runtime & Integration Infrastructure ».
- [ ] Gist de wincent « List of coding agent sandboxes » : commentaire.
- [ ] PR sur restyler/awesome-sandbox, webcoyote/awesome-AI-sandbox,
  bureado/awesome-agent-runtime-security.
- [ ] Recomp Software, liste PlayStation : demander l'ajout de CTR Native
  Web.
- [ ] Discord CTR-tools : présenter le port web, créditer ctr-native et
  OnlineCTR, demander des testeurs.
- [ ] Dev.to : activer « Publish from RSS » sur le flux Pages si un blog
  naît.

## E. À vérifier au moment (non confirmé dans la recherche)

- fosstodon.org sur invitation ou non ; sinon hachyderm.io.
- Seuil de karma r/ClaudeAI (50 d'après les agrégateurs) et règles de
  r/ClaudeCode, r/vscode, r/emulation : relire la sidebar avant de poster.
- Handle Bluesky via `meitogi.github.io` (section A).
- Limite d'image Bluesky : 1 000 000 octets, 4 images max, alt obligatoire.
