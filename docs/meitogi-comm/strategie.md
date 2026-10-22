# Stratégie — faire connaître Meitogi

## 1. Positionnement et voix

**Meitogi, c'est** : « open-source tooling for working seriously with AI
coding agents, and a few things built for fun ». Un dev français qui build
in public, **en anglais**, avec des captures réelles plutôt que des visuels
marketing, de l'humour sec, et uniquement des affirmations vérifiables sur
GitHub (règle déjà posée dans la candidature Claude for OSS).

Trois produits, trois audiences, un seul compte par réseau :

| Produit | Promesse en une ligne | Audience |
|---|---|---|
| Dev Sandbox (image + CLI) | A devcontainer that gives Claude Code room to work, and a network it cannot leave | devs Claude Code, devs sécurité, équipes sous NDA |
| CTR Native Web | Crash Team Racing in your browser, with online play. Bring your own disc | retro gaming, décompilation, WebAssembly |
| PurrPause | A break reminder with a dancing cat, built for teenagers' PCs | parents, LinkedIn FR |

## 2. Réseaux, par priorité

| Rang | Réseau | Rôle | Cadence | Automatisé ? |
|---|---|---|---|---|
| 1 | GitHub + awesome lists | la vraie surface de découverte de Dev Sandbox | one-shot | non (formulaires) |
| 2 | Bluesky | réseau principal : pas de pénalité lien, feeds thématiques, stats gratuites | 2 / semaine | oui, direct depuis la page |
| 3 | Mastodon (fosstodon ou hachyderm) | miroir de Bluesky avec 3 à 5 hashtags | 2 / semaine | oui, direct depuis la page |
| 4 | LinkedIn | post natif (image ou carrousel PDF, jamais de lien) ; réseau FR existant | 1 / semaine | non : bouton « copier » dans la page |
| 5 | Discord CTR-tools, r/ClaudeAI, r/SideProject | événementiel, après avoir répondu à des questions | au lancement | non |
| 6 | Hacker News | trois Show HN dans l'année | 1 / trimestre max | non |
| 7 | Dev.to | un article de fond par trimestre | 1 / trimestre | oui, via l'Action |
| 8 | Product Hunt | PurrPause seulement, lancement gratuit | one-shot | non |
| 9 | YouTube Shorts | archive des GIF de démo, pas de chaîne | quand il y a un GIF | non (upload Studio) |
| 10 | X | handle réservé ; 1 post / semaine pré-rempli par intent, lien en réponse ; **test de 3 mois** tranché sur les referrers GitHub | 1 / semaine | non : intent + Ctrl+V |
| — | Threads | handle réservé, rien d'autre | — | — |

**Exclus de l'automatisation et pourquoi** : X (API payante à l'acte,
liens à 0 engagement médian pour un compte gratuit, dashboard payant ;
reste accessible à la main, pré-rempli), Hashnode (API Pro),
API Reddit (formulaire d'approbation), API YouTube (vidéos forcées en
privé avant audit), Buffer / Typefully / Publer / Postiz hébergé (inutiles
dès que la page publie elle-même), Mixpost (ni Bluesky ni API en Lite).

## 3. Piliers de contenu et ratio

| Pilier | Part | Exemples |
|---|---|---|
| Montrer le travail | 40 % | GIF 5 à 15 s d'une feature, avant/après, extrait de changelog, capture de terminal |
| Conseils pratiques | 30 % | une règle de firewall expliquée, un hook Claude Code, un piège devcontainer, un « TIL » |
| Humour | 20 % | capture réelle de Claude faisant un truc absurde dans la sandbox, meme memegen |
| Produit et lancement | 10 % | release, Show HN, nouvelle page |

Le ratio 80/20 valeur/promo est la norme sur tous les réseaux. Un post
d'humour au maximum par lot ; jamais deux posts produit de suite.

## 4. Rythme hebdomadaire (2 h)

1. **45 min, rédaction** : lancer le prompt de posts ([prompt-posts.md](prompt-posts.md)),
   relire les brouillons dans la page, ajuster, programmer. Un seul visuel
   par semaine (GIF, capture, carte générée).
2. **45 min, conversation** : répondre à chaque commentaire ; répondre à
   2 ou 3 questions sur r/ClaudeAI, le Discord CTR-tools ou HN. C'est ce
   qui construit le karma et les 90 % de non-promo exigés sur Reddit.
3. **15 min, divers** : publier le post LinkedIn et le meilleur post de la
   semaine sur X depuis les boutons pré-remplis, uploader un Short s'il y
   a un GIF, noter 2 idées dans `ideas.md`.
4. **15 min par mois** : le bilan (section 7).

Tout ce qui n'est pas dans cette liste ne se fait pas.

## 5. Séquence de lancement

Une étape par trimestre au plus, pour pouvoir tenir le fil de commentaires
pendant 6 h le jour J. Dans l'ordre :

1. **Mise en place** : comptes, page, premier lot de 6 posts, profil
   README `meitogi/meitogi`. Voir [preparation.md](preparation.md).
2. **Listes et communautés** (2 h, une fois) : awesome-claude-code
   (formulaire web, projet de plus de 14 jours ou 100 étoiles), gist de
   wincent « coding agent sandboxes », PR sur restyler/awesome-sandbox,
   Recomp Software pour CTR, présentation de CTR Native Web sur le Discord
   CTR-tools en créditant l'upstream.
3. **Show HN : CTR Native Web**. Le meilleur candidat : jouable
   immédiatement, WASM + online, HN adore ça. Inclure l'URL live, le repo,
   3 à 5 lignes sur le comment (port Emscripten, WebRTC, relais), les
   limites connues, la mention « requires your own game files ».
4. **Article Dev.to** : « How the deny-by-default DNS firewall in Dev
   Sandbox works », republié en GitHub Discussion.
5. **Show HN : Dev Sandbox**, après l'article et une release propre.
6. **Product Hunt : PurrPause**, lancement gratuit, visuel, sans boost.

Règles Show HN : titre commençant par « Show HN: », quelque chose à
essayer sans inscription, jamais de demande de vote, répondre à chaque
question. Un Show HN sans traction peut être resoumis plus tard.

## 6. Garde-fous par produit

- **PurrPause** : jamais « hidden », « camouflaged », « hard to disable ».
  Dire : « runs as a service so it survives reboots and can't be closed by
  accident », « it reminds, it does not monitor », « nothing leaves the
  PC », « open source, documented uninstall ». La frontière avec le
  stalkerware est exactement la discrétion : on reste du bon côté par le
  vocabulaire et par la transparence (nom visible, overlay visible).
- **CTR Native Web** : toujours « bring your own NTSC-U disc, nothing is
  hosted », créditer CTR-tools / ctr-native / OnlineCTR, wordmark inspiré
  et non logo officiel, « Crash Team Racing is a trademark of Sony
  Interactive Entertainment ». Pas de vidéo longue de gameplay en avant
  sans ce rappel.
- **Dev Sandbox** : uniquement des faits vérifiables (tests de privilège
  rejouables, DNS REFUSED, extension non modifiée). Pas de « claude-code »
  dans le nom. Se positionner face à Docker Sandboxes et au devcontainer
  de référence Anthropic sur le concret, pas sur l'adjectif.

## 7. Langue

Anglais partout, un compte par réseau. Les indés français qui marchent à
l'international postent en anglais et signalent l'origine par un drapeau
dans la bio (Marc Lou, Thomas Sanlis, Florian Rival, Rémi Verschelde).
Français uniquement sur LinkedIn, et seulement pour PurrPause ou les posts
destinés au réseau pro local. Pas de comptes séparés FR : double temps,
fraction de la portée.

## 8. Mesure : 15 min par mois, quatre lignes dans `metrics.md`

1. Delta d'étoiles par repo et top 3 des referrers GitHub (Insights →
   Traffic). Le seul chiffre qui relie les réseaux à un résultat.
2. Par réseau : delta d'abonnés et le meilleur post du mois (format ×
   pilier).
3. Réponses reçues vs envoyées.
4. Une décision : quel format doubler ; quel réseau couper s'il n'a rien
   envoyé depuis 3 mois.

Sources gratuites : GitHub Insights, Bluesky Meter / Graphtracks, LinkedIn
natif, dashboard Dev.to, YouTube Studio. Mastodon : compteurs par post
seulement.
