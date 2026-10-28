# Comptes Meitogi — le kit de création

Tout ce qu'il faut pour créer les comptes à la chaîne : le lien direct,
le handle, le nom, la bio à coller, l'image à charger, puis la clé à
récupérer et où elle va. Complète [../preparation.md](../preparation.md)
(qui garde le pourquoi) ; ici, uniquement le quoi-cliquer.

Disponibilités et règles d'inscription vérifiées le **2026-10-28**.

## Identité commune

| Champ | Valeur |
|---|---|
| Handle | `meitogi` partout (sinon `meitogi_dev`, le même partout où il faut un repli) |
| Nom affiché | `Meitogi` |
| Lien | `https://github.com/meitogi` |
| Adresse mail | la même partout ; noter chaque mot de passe dans le gestionnaire au fil de l'eau |

### Images ([images/](images/))

| Fichier | Pour |
|---|---|
| [avatar-400.png](images/avatar-400.png) | avatar partout où 400 suffit (X, Mastodon, LinkedIn, Facebook, Reddit, HN n'en a pas) |
| [avatar-1024.png](images/avatar-1024.png) | Bluesky, Instagram/Threads, TikTok, YouTube (800 mini), Discord, Dev.to, Product Hunt |
| [banner-1500x500.png](images/banner-1500x500.png) | X, Mastodon |
| [banner-3000x1000.png](images/banner-3000x1000.png) | Bluesky (3:1, plus net) |
| [linkedin-cover-1128x191.png](images/linkedin-cover-1128x191.png) | couverture de la page entreprise LinkedIn |
| [facebook-cover-851x315.png](images/facebook-cover-851x315.png) | couverture de la page Facebook (taille recommandée par l'aide Facebook) |
| [youtube-banner-2560x1440.png](images/youtube-banner-2560x1440.png) | bannière de chaîne (le logo tient dans la zone sûre 1546×423) |

Tout est sur le fond acier de l'avatar (`#161A20`), croissant et texte
argent : profil et bannière se lisent d'un bloc. Les bannières X et
Bluesky gardent le logo à droite du centre (l'avatar recouvre le coin
bas-gauche). Source : `apps/megaphone/assets/meitogi-avatar.svg`.

**Charger une image depuis la fenêtre « Ouvrir » du Mac** : dans le
sélecteur de fichiers du navigateur, **⌘ ⇧ G** (« Aller au dossier »),
coller le chemin ci-dessous, **Entrée**, puis choisir le fichier.

```
/Volumes/Data/dev/devcontainer-tools-v2/docs/meitogi-comm/comptes/images
```

Le sélecteur garde ce dossier pour les uploads suivants du même
navigateur : le raccourci ne sert qu'une fois par navigateur.

### Bios (comptées, toutes sous la limite du réseau)

**Longue** — Bluesky (228 / 256) :

```
Open-source dev tools, retro games in the browser, and things built for fun. Dev Sandbox (a devcontainer Claude Code cannot leave), Crash Team Racing in your browser, a dancing-cat break reminder. Building in public from France.
```

**Mastodon** (374 / 500, sauts de ligne compris) :

```
Open-source dev tools, retro games in the browser, and things built for fun.

Dev Sandbox: a devcontainer that gives Claude Code room to work, and a network it cannot leave.
CTR Native Web: Crash Team Racing in your browser, online play, bring your own disc.
PurrPause: a break reminder with a dancing cat.

Building in public from France. #opensource #devtools #webassembly
```

**Courte** — X (160), Instagram (150), Threads (150) : 116 caractères

```
Open-source dev tools, retro games in your browser and a dancing-cat break reminder. Building in public from France.
```

**TikTok** (74 / 80) :

```
Dev tools, retro games in your browser, a dancing cat. Open source, France.
```

**Accroche** — slogan LinkedIn (75 / 120), bio de la page Facebook (75 / 101), Product Hunt, et le texte des bannières :

```
Open-source dev tools, retro games in the browser, and things built for fun
```

**Dev.to** (148 / 200), aussi pour la description YouTube et LinkedIn :

```
Open-source dev tools, retro games in the browser, and things built for fun: Dev Sandbox, CTR Native Web, PurrPause. Building in public from France.
```

**Reddit** (122 / 200), aussi le champ « about » de Hacker News :

```
Solo dev from France. Open-source dev tools (Dev Sandbox), retro games in the browser (CTR), a dancing-cat break reminder.
```

Le drapeau dans la bio (stratégie § 7) reste à ton choix : ajoute-le à
la main, chaque bio garde la marge.

### Liens de profil (Mastodon : 4 champs ; Bluesky : section « Links » ; ailleurs, le premier seulement)

| Libellé | URL |
|---|---|
| GitHub | `https://github.com/meitogi` |
| Dev Sandbox | `https://github.com/meitogi/devcontainer-sandbox` |
| CTR Native Web | `https://ctr.suuuper.app` |
| PurrPause | `https://github.com/meitogi/PurrPause` |

## Handles au 2026-10-28

| Réseau | `meitogi` | Comment |
|---|---|---|
| Bluesky | libre | `meitogi.bsky.social` ne résout pas |
| Mastodon | libre sur hachyderm.io, mastodon.social, fosstodon.org | API `accounts/lookup` → 404 |
| Dev.to | libre | API → 404 |
| GitHub | **à toi** (compte utilisateur, 14 repos publics) ; `megaphone`, `meitogi` et `meitogi.github.io` n'existent pas encore | API |
| X | pris → **`meitogi_dev`** | choisi à l'inscription |
| Instagram, Threads | pris → **`meitogi_dev`** (Threads suit Instagram) | choisi à l'inscription |
| TikTok, Reddit, HN, Product Hunt, YouTube | non vérifiable d'ici | le formulaire d'inscription le dit |

## Ordre

1. **Aujourd'hui, 45 min** : Bluesky, Mastodon, Discord, Dev.to, GitHub.
   Les clés de ces cinq-là suffisent au premier post réel.
2. **Aujourd'hui aussi, les demandes longues** : Reddit (date limite
   possible le 31 octobre, voir plus bas) et l'audit YouTube.
3. **Réserver les handles** sans rien publier : X, TikTok, Hacker News,
   Product Hunt, Lobste.rs.
4. **Après le push du repo** (l'URL Pages sert de redirect) : Meta
   (Threads, Facebook, Instagram) puis LinkedIn, avec les deux assistants
   `npm run meta:auth` et `npm run linkedin:auth`.
5. **Une fois tout créé, les semaines suivantes** : faire vivre les
   comptes avant le premier post vitrine — karma Reddit avec Scout,
   commentaires sur Hacker News (section 5).

Chaque clé va dans les **secrets du repo**
(https://github.com/meitogi/megaphone/settings/secrets/actions, une fois
le repo créé) et dans `apps/megaphone/.env` pour la ligne de commande ;
le récapitulatif est en fin de document.

---

## 1. Les cinq du premier post

### Bluesky

- [ ] Créer : https://bsky.app → « Create account », hébergeur Bluesky
      Social, handle `meitogi` (donne `meitogi.bsky.social`)
- [ ] Profil (sur ta page de profil → « Edit profile ») : nom `Meitogi`,
      bio **longue**, avatar `avatar-1024.png`, bannière
      `banner-3000x1000.png`
- [ ] Liens (même écran, section « Links », sous la bio depuis le
      2026-10-22) : les 4 liens de profil, le libellé dans « title »,
      GitHub en premier (l'ordre se règle en glissant)
- [ ] Clé : https://bsky.app/settings/app-passwords → « Add App
      Password », nom `megaphone`, sans accès aux messages privés
- Secrets : `BLUESKY_HANDLE` = `meitogi.bsky.social`,
  `BLUESKY_APP_PASSWORD` = le mot de passe affiché (une seule fois)
- Plus tard, gratuit : handle `@meitogi.github.io` via le repo
  `meitogi/meitogi.github.io` (preparation.md § A).

### Mastodon

**fosstodon.org est sur invitation** (son API d'instance renvoie
« registrations enabled : false »). Choix, dans l'ordre :

1. **hachyderm.io** — tech, inscriptions ouvertes **sur validation** avec
   un motif (18 ans et plus) ;
2. **mastodon.social** — ouvert sans validation, si hachyderm tarde.

- [ ] Créer : https://hachyderm.io/auth/sign_up (ou
      https://mastodon.social/auth/sign_up), handle `meitogi`, nom
      `Meitogi`. Motif à coller :

      ```
      Solo open-source developer from France. I'd like to share and discuss my projects here: a hardened devcontainer for AI coding agents, a WebAssembly port of a PS1 racing game decompilation, and small dev tools. Posts in English about what I build, no automation beyond scheduling my own posts.
      ```

- [ ] Profil : https://hachyderm.io/settings/profile — bio **Mastodon**,
      avatar `avatar-400.png`, en-tête `banner-1500x500.png`, les 4 liens
      de profil ; cocher « This is an automated account » **non** (c'est
      toi qui écris, l'outil programme seulement)
- [ ] Clé : https://hachyderm.io/settings/applications/new — nom
      `megaphone`, décocher tout, cocher `write:statuses`, `write:media`,
      `read:accounts` → « Submit » → ouvrir l'appli → « Your access
      token »
- Secrets : `MASTODON_URL` = `https://hachyderm.io` (ou
  `https://mastodon.social`), `MASTODON_TOKEN` = le jeton

### Discord

- [ ] Un serveur à toi (en créer un : app Discord → « + » → « Create My
      Own »), nom `Meitogi`, icône `avatar-1024.png`
- [ ] Un salon texte `announcements` → roue dentée → Integrations →
      Webhooks → « New Webhook », nom `Meitogi`, avatar `avatar-400.png`
      → « Copy Webhook URL »
- Secret : `DISCORD_WEBHOOK` = l'URL copiée

### Dev.to

- [ ] Créer : https://dev.to/enter?state=new-user (« Sign up with
      GitHub » sur le compte `meitogi` le plus simple), username `meitogi`
- [ ] Profil : https://dev.to/settings/profile — nom `Meitogi`, bio
      **Dev.to**, avatar `avatar-1024.png`, site
      `https://github.com/meitogi`
- [ ] Clé : https://dev.to/settings/extensions → « DEV Community API
      Keys », description `megaphone` → « Generate API Key »
- Secret : `DEVTO_API_KEY`

### GitHub

- [ ] Repo : https://github.com/new?owner=meitogi&name=megaphone&visibility=public&description=Write%2C%20preview%2C%20schedule%20and%20publish%20social%20posts%20on%20every%20network%20from%20one%20static%20page.%20The%20repo%20is%20the%20queue%2C%20GitHub%20Actions%20the%20scheduler%2C%20Pages%20the%20UI.%20Zero%20cost.
      — le lien pré-remplit la description ; **vide** (ni README, ni
      licence, ni .gitignore : le repo local a déjà son historique),
      puis, **avant le push**, Pages :
      https://github.com/meitogi/megaphone/settings/pages → Source
      « GitHub Actions » (sinon le premier déploiement échoue en 404 :
      « Ensure GitHub Pages has been enabled ») ; puis pousser
      `apps/megaphone/` depuis Fork (README de l'app, § Publishing this
      repository). Description (166 / 350) :

      ```
      Write, preview, schedule and publish social posts on every network from one static page. The repo is the queue, GitHub Actions the scheduler, Pages the UI. Zero cost.
      ```

- [ ] « About » du repo (roue dentée à droite de
      https://github.com/meitogi/megaphone, une fois poussé) : Website =
      `https://meitogi.github.io/megaphone/`, cocher « Use your GitHub
      Pages website » ; Topics :

      ```
      social-media scheduler github-actions github-pages bluesky mastodon threads linkedin static-site zero-cost
      ```

- [ ] **Deux** jetons fine-grained, https://github.com/settings/personal-access-tokens/new,
      « Only select repositories » → `meitogi/megaphone`, expiration 1 an :
  - `megaphone-page` : Contents **read/write**, Actions **read/write** →
    se colle dans le panneau Réglages de la page (navigateur), jamais
    en secret ;
  - `megaphone-refresh` : Secrets **read/write** → secret `GH_PAT`
    uniquement. Séparés, pour que le jeton qui vit dans le
    `localStorage` du navigateur ne puisse pas réécrire les secrets.
- [ ] Optionnel : profil README, https://github.com/new?owner=meitogi&name=meitogi&visibility=public
      (preparation.md § A) ; bio GitHub, si tu veux remplacer l'actuelle
      (108 / 160) :

      ```
      Open-source dev tools, retro games in the browser, and things built for fun. Building in public from France.
      ```

## 2. Les demandes longues (à déposer maintenant)

### Reddit

- [ ] Compte : https://www.reddit.com/register/ — username `meitogi`,
      avatar `avatar-400.png`, bio **Reddit** (Settings → Profile)
- [ ] Profil complet, https://www.reddit.com/settings/profile : un
      profil vide sur un compte neuf est le premier indice « bot ».
      - *Display name* → `Meitogi`
      - *About* (183 / 200), la bio Reddit plus une ligne de
        transparence, qui répond d'avance à « bot » ; elle dit
        « draft » aussi, pas seulement « polish », parce que c'est ce
        que fait Scout :

        ```
        Solo dev from France. Open-source dev tools (Dev Sandbox), retro games in the browser (CTR), a dancing-cat break reminder. English is my second language, AI helps me draft and polish.
        ```

      - *Banner* → `banner-1500x500.png` (Reddit recadre au centre)
      - *Social links* → GitHub `https://github.com/meitogi`, puis
        Bluesky une fois le compte créé
- [x] Demande d'accès API — **refusée le 2026-10-28** (réponse type « Responsible
      Builder Policy and/or lacks necessary details »). Pas de nouvelle
      demande : rien n'en dépend. Gardé pour mémoire. Depuis novembre 2025, plus de création d'app en
      libre-service : formulaire du support développeur,
      https://support.reddithelp.com/hc/en-us/requests/new. Une source
      tierce, **non confirmée**, dit que Reddit cesse d'en accepter le
      **31 octobre 2026**. Refus fréquents ; rien ne dépend de la réponse
      (Reddit reste en post pré-rempli dans Megaphone). Le formulaire
      demande un lien vers le code : **pousser `meitogi/megaphone` avant**
      si possible, sinon le lien est un 404.

      Champ par champ :

      - *What is your inquiry?* → **I'm a developer and want to build a
        Reddit App that does not work in the Devvit ecosystem.**
      - *Reddit account name* → `meitogi`
      - *What benefit/purpose will the bot/app have for Redditors?*

        ```
        Honestly, the same as a post made by hand: an open-source developer sharing his own projects (a hardened devcontainer for AI coding agents, a WebAssembly port of a PS1 racing game decompilation, small dev tools) in the communities where they are on-topic. The app adds no content of its own; it only lets me publish a post I already wrote and reviewed at a set time, alongside the other networks I post to, instead of copy-pasting it by hand. No engagement automation of any kind.
        ```

      - *Provide a detailed description of what the Bot/App will be doing
        on the Reddit platform.*

        ```
        A personal "script" app used by one account, u/meitogi, from my publishing tool (Megaphone: a GitHub repository where each post is a Markdown file, published by a GitHub Actions workflow).

        What it does:
        - Authenticates as u/meitogi with a script-type OAuth app.
        - Submits a self or link post to one subreddit (POST /api/submit), only for posts I wrote and reviewed myself, only in subreddits whose rules allow sharing open-source work, after I read the sidebar.
        - Reads back the permalink of that post (GET /api/info) so my tool records where it was published.

        What it never does: read or collect other users' content or profiles, vote, comment, send messages, follow, or post to a subreddit I have not chosen for that post.

        Example: a "Show-and-tell" post on r/ClaudeAI about a new release of my devcontainer, title and body written by me, scheduled for Tuesday 15:00 UTC; the workflow submits it once and stores its URL.

        Volume: a few posts per quarter, under 20 API requests per month.
        ```

      - *What is missing from Devvit that prevents building on that
        platform?*

        ```
        Devvit apps act inside the subreddits that install them, and user actions (posting as the user) must come from an explicit manual action in Reddit's UI; automated posting is ruled out. My use case is the opposite: a post I wrote, published at a scheduled time to a community I do not moderate, triggered from outside Reddit by a GitHub Actions workflow. Devvit has no way to do that without a moderator installing my app in each target subreddit, which would be inappropriate for an occasional self-post.
        ```

        Vérifié sur la doc Devvit (pages Reddit API et User Actions :
        « only on an explicit manual action », « No automated actions »).

      - *Provide a link to source code or platform that will access the
        API.* → `https://github.com/meitogi/megaphone`
      - *What subreddits do you intend to use the bot/app in?*

        ```
        r/ClaudeAI, r/ClaudeCode, r/devcontainers, r/docker, r/emulation, r/WebAssembly, r/vscode — only where the rules allow it, checked before each post.
        ```

      - *If applicable, what username will you be operating this Bot/App
        under?* → `meitogi`

### YouTube

- [ ] Chaîne : https://www.youtube.com/account → « Create a channel » /
      « Add or manage your channel(s) » → nouvelle chaîne au nom
      `Meitogi` (compte de marque), handle `@meitogi`
- [ ] Personnalisation : https://studio.youtube.com → Customization —
      photo `avatar-1024.png`, bannière `youtube-banner-2560x1440.png`,
      description **Dev.to**, lien GitHub
- [ ] Projet Google Cloud : https://console.cloud.google.com/projectcreate
      (`meitogi-megaphone`), puis activer l'API :
      https://console.cloud.google.com/apis/library/youtube.googleapis.com
      → « Activer »
- [ ] Écran de consentement OAuth (Google Auth Platform, assistant en
      étapes), champ par champ :
  1. **Informations sur l'application** — nom `Meitogi megaphone` ;
     adresse d'assistance : ton adresse (celle du compte Google) ; **pas
     de logo** : en importer un oblige à faire valider l'app dès qu'elle
     n'est plus en « Test ».
  2. **Audience** — **Externe** (« Interne » n'existe que pour un compte
     Google Workspace).
  3. **Coordonnées du développeur** — ton adresse.
  4. **Niveaux d'accès** (facultatif) — « Ajouter » →
     `https://www.googleapis.com/auth/youtube.upload`, rien d'autre
     (scope « sensible » : c'est lui qui déclenche l'avertissement
     « application non validée »).
  5. **ID client OAuth** — type **« Application de bureau »** (« Desktop
     app »), nom `megaphone-cli`.
  6. **Vos identifiants** — télécharger le JSON (client id + secret) ;
     il va dans `apps/megaphone/.env` le jour où l'upload existe, jamais
     dans le repo.
- [ ] Ensuite, menu **Audience** : ajouter ton compte Google en
      **utilisateur test** ; puis **« Publier l'application »** (état « En
      production ») une fois l'upload codé. En « Test », le jeton de
      rafraîchissement expire au bout de **7 jours** ; en production sans
      validation, Google affiche « application non validée » à ton
      consentement (« Paramètres avancés » → continuer), plafond 100
      utilisateurs, et le jeton n'expire plus — c'est le bon réglage pour
      un outil perso.
- [ ] Audit : https://support.google.com/youtube/contact/yt_api_form —
      sans lui, toute vidéo envoyée par l'API reste **privée**.
      Plusieurs semaines ; d'ici là, upload depuis Studio. **Aucune session
      de Megaphone ne code l'upload YouTube aujourd'hui** (Shorts = MP4
      téléchargé + upload Studio) : l'audit demande de décrire une app qui
      existe, donc le déposer maintenant risque un refus. Créer projet et
      client OAuth ne coûte rien ; l'audit attend que l'upload soit codé.

## 3. Réserver les handles

Profil complet, rien de publié.

| Réseau | Créer | Bio | Avatar | Bannière |
|---|---|---|---|---|
| X | https://x.com/i/flow/signup puis https://x.com/settings/profile | courte | `avatar-400.png` | `banner-1500x500.png` |
| TikTok | https://www.tiktok.com/signup (le profil se règle dans l'app) | TikTok | `avatar-1024.png` | — |
| Hacker News | https://news.ycombinator.com/login → « Create Account » ; le champ « about » sur ta page user | Reddit | — | — |
| Product Hunt | https://www.producthunt.com → « Sign up » | accroche | `avatar-1024.png` | — |
| Lobste.rs | sur invitation : https://lobste.rs/chat, demander poliment | — | — | — |

## 4. Meta et LinkedIn (après le push)

### Meta — Instagram, Facebook, Threads

Dans cet ordre, car chaque étape s'appuie sur la précédente :

- [ ] **Instagram** : https://www.instagram.com/accounts/emailsignup/ —
      username **`meitogi_dev`** (créé ; connexion desktop refusée le
      2026-10-28, à retenter le lendemain), nom `Meitogi`, bio **courte**, avatar
      `avatar-1024.png`, lien GitHub ; puis Settings → « Account type and
      tools » → **Switch to professional account** (Creator ou Business)
- [ ] **Page Facebook** : https://www.facebook.com/pages/create — nom
      `Meitogi`, catégorie « Software », bio = **accroche**, photo
      `avatar-400.png`, couverture `facebook-cover-851x315.png`, site
      `https://github.com/meitogi` ; nom d'utilisateur de page `meitogi`
- [ ] **Lier** Instagram à la page : paramètres de la page → Linked
      accounts → Instagram
- [ ] **Threads** : https://www.threads.com → se connecter avec Instagram
      `meitogi_dev` (le handle suit), importer la bio
- [ ] **App Meta** : https://developers.facebook.com/apps/creation/ — nom
      `Meitogi megaphone`, cas d'usage « Access the Threads API »,
      « Manage everything on your Page » (Facebook Login) et « Manage
      messaging & content on Instagram » ; App Roles → ton compte en
      testeur Threads (et accepter l'invitation côté Threads, Settings →
      Account → Website permissions)
- [ ] Jetons : `cd apps/megaphone && npm run meta:auth` — l'assistant
      demande app id + secret, fait les échanges et pose les secrets
- Secrets (posés par l'assistant) : `THREADS_USER_ID`, `THREADS_TOKEN`,
  `FB_PAGE_ID`, `FB_PAGE_TOKEN`, `IG_USER_ID`

### LinkedIn

L'outil publie **sur ton profil personnel** (scope `w_member_social`,
secret `LINKEDIN_PERSON_URN`) ; la page entreprise sert d'abord à
héberger l'app, que LinkedIn exige rattachée à une page.

Tout se fait depuis **ton vrai compte** : LinkedIn refuse de créer une
page depuis un profil sans relations, et un profil perso au nom
« Meitogi » enfreint ses règles (nom réel exigé) — supprime « Meitogi
dev ». Conséquence : les posts de l'outil sortent sous ton nom.

- [x] Page entreprise (créée le 2026-10-28) : https://www.linkedin.com/company/setup/new/ —
      nom `Meitogi`, URL `linkedin.com/company/meitogi`, site
      `https://github.com/meitogi`, slogan = **accroche**, logo
      `avatar-400.png`, couverture `linkedin-cover-1128x191.png`,
      description **Dev.to**
- [ ] App : https://www.linkedin.com/developers/apps/new — nom `Meitogi
      megaphone`, page `Meitogi`, logo `avatar-400.png` ; onglet
      Products → « Share on LinkedIn » et « Sign In with LinkedIn using
      OpenID Connect » ; onglet Settings → « Verify » (lien à ouvrir
      en admin de la page) ; onglet Auth → redirect URL =
      `https://meitogi.github.io/megaphone/`
- [ ] Jeton : `cd apps/megaphone && npm run linkedin:auth` (client id +
      secret de l'onglet Auth) ; à refaire tous les 60 jours,
      `refresh-tokens.yml` prévient 10 jours avant
- Secrets (posés par l'assistant) : `LINKEDIN_PERSON_URN`,
  `LINKEDIN_TOKEN`, `LINKEDIN_EXPIRES`

## 5. Faire vivre les comptes (une fois tout créé)

Rien de vitrine tant que les comptes n'ont pas un peu d'historique.

- [ ] **Reddit, ~50 karma sur r/ClaudeAI** avant tout post vitrine, en
      répondant à de vraies questions. **Scout** (README de l'app,
      § Scout) : un userscript sur reddit.com qui liste les fils
      récents de tes 7 subreddits où tu as quelque chose à dire, et
      prépare un brouillon via `npm run ui`. Il ne poste rien : tu relis,
      tu mets à ta façon, tu postes. Quelques réponses par semaine
      suffisent, pas de rafale.
- [ ] **Hacker News** : commenter un peu ; un compte qui a un historique
      passe mieux pour le premier Show HN. Le même Scout marche sur
      news.ycombinator.com, **mais HN interdit le texte généré ou retouché
      par IA** (« Don't post generated text or AI-edited text ») : le
      brouillon y sert de notes, le commentaire s'écrit à la main.
- [ ] **Lobste.rs**, si l'invitation arrive : 70 jours sans auto-promo.
- [ ] **Product Hunt** : suivre quelques lancements avant celui de
      PurrPause.

---

## Récapitulatif des secrets

https://github.com/meitogi/megaphone/settings/secrets/actions — et les
mêmes valeurs dans `apps/megaphone/.env` (gitignoré) pour la ligne de
commande.

| Secret | Vient de | Posé par |
|---|---|---|
| `BLUESKY_HANDLE`, `BLUESKY_APP_PASSWORD` | Bluesky | toi |
| `MASTODON_URL`, `MASTODON_TOKEN` | Mastodon | toi |
| `DISCORD_WEBHOOK` | Discord | toi |
| `DEVTO_API_KEY` | Dev.to | toi |
| `GH_PAT` | jeton `megaphone-refresh` | toi |
| `THREADS_USER_ID`, `THREADS_TOKEN` | Meta | `npm run meta:auth` |
| `FB_PAGE_ID`, `FB_PAGE_TOKEN`, `IG_USER_ID` | Meta | `npm run meta:auth` |
| `LINKEDIN_PERSON_URN`, `LINKEDIN_TOKEN`, `LINKEDIN_EXPIRES` | LinkedIn | `npm run linkedin:auth` |

Le panneau Réglages de la page (par navigateur) prend : le jeton
`megaphone-page`, le handle + app password Bluesky, l'URL + jeton
Mastodon, le webhook Discord.

Un réseau sans secret est simplement sauté : chaque compte s'ajoute
quand il est prêt, sans rien bloquer.
