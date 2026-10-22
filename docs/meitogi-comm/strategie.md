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

## 2. Réseaux : partout, automatisé au maximum

Le même post part partout. La seule différence entre réseaux est le niveau
d'automatisation, donc le temps par post : zéro pour ceux que l'outil
publie, trente secondes pour ceux ouverts pré-remplis, trois minutes pour
un upload vidéo.

| Réseau | Automatisation | Temps / post | Mise en place, une fois | Rôle |
|---|---|---|---|---|
| Bluesky | directe depuis la page, et Action | 0 | 10 min | principal dev |
| Mastodon | directe et Action | 0 | 10 min | miroir dev, OSS, sécurité |
| Threads | Action (API gratuite, app Meta, jeton rafraîchi par l'Action) | 0 | 1 h, avec Facebook et Instagram | indés, grand public |
| Facebook, page | Action (API Graph, même app Meta, jeton de page sans expiration) | 0 | compris ci-dessus | PurrPause, parents FR |
| Instagram, compte pro | Action (même app ; image obligatoire, sinon le post saute) | 0 | compris ci-dessus | PurrPause, visuels CTR |
| LinkedIn | Action (API gratuite, page entreprise ; reconsentement de 2 min tous les 60 jours) | 0 | 1 h | pro FR, Dev Sandbox |
| Discord | webhook | 0 | 5 min | annonces |
| Dev.to | Action | 0 | 5 min | un article par trimestre |
| X | intent pré-rempli, Ctrl+V (API payante) | 30 s | 5 min | portée loterie, test de 3 mois |
| Reddit | intent pré-rempli ; demande d'accès API déposée (formulaire Responsible Builder, gratuit, souvent refusé) | 30 s | 10 min | vitrines, règle 90/10 |
| Hacker News | intent pré-rempli | 30 s | 5 min | Show HN |
| YouTube Shorts | upload Studio ; audit API demandé (gratuit) pour automatiser plus tard | 3 min | 10 min | archive des GIF, recherche |
| TikTok | upload depuis l'app ; API réservée aux apps auditées | 3 min | 5 min | GIF CTR, humour |
| Product Hunt | formulaire | one-shot | 5 min | PurrPause |
| GitHub + awesome lists | formulaires | one-shot | 2 h | découverte de Dev Sandbox |

Ordre de mise en place : Bluesky, Mastodon et Discord d'abord (session 2
de l'outil) ; puis l'Action, les intents et Dev.to (session 3) ; puis
l'app Meta et LinkedIn (session 4). Les demandes d'accès YouTube et
Reddit se déposent dès le début : elles prennent des semaines et ne
coûtent rien.

**Hors périmètre, et pourquoi** : l'API X (payante à l'acte), Hashnode
(API Pro depuis juin 2026), Buffer, Typefully, Publer, Postiz hébergé
et Mixpost (inutiles dès que l'outil publie lui-même).

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

Deux posts par semaine, publiés partout à la fois. LinkedIn et Facebook
reçoivent la variante longue, Instagram seulement les posts qui ont une
image, Dev.to seulement les articles.

1. **45 min, rédaction** : lancer le prompt de posts ([prompt-posts.md](prompt-posts.md)),
   relire les brouillons dans la page, ajuster, programmer. Un seul visuel
   par semaine (GIF, capture, carte générée).
2. **45 min, conversation** : répondre à chaque commentaire ; répondre à
   2 ou 3 questions sur r/ClaudeAI, le Discord CTR-tools ou HN. C'est ce
   qui construit le karma et les 90 % de non-promo exigés sur Reddit.
3. **15 min, divers** : ouvrir les boutons pré-remplis (X, et Reddit ou
   HN s'il y a lieu), uploader le GIF de la semaine en Short et sur
   TikTok, noter 2 idées dans `ideas.md`.
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
Français uniquement dans la variante longue de LinkedIn et Facebook, et
seulement pour PurrPause ou les posts destinés au réseau local. Pas de comptes séparés FR : double temps,
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
