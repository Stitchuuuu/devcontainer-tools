# Captures à faire

Une capture par post, jamais un visuel marketing : un écran réel, un
terminal réel. Liste de départ, à cocher ; elle alimente le premier lot et
le profil README.

## Règles communes

- **Base 16:9 en 1600×900**, thème sombre partout, zoom éditeur 1.5× pour
  que le texte reste lisible en vignette. La page et `card.mjs` dérivent
  les autres formats : 1200×675 (Bluesky, X, LinkedIn), 1080×1350 (Instagram
  4:5), 1080×1920 (Shorts, TikTok, recadrage centré ou fond flou).
- **GIF ≤ 15 s, exporté aussi en MP4** : Bluesky et Mastodon acceptent la
  vidéo ; un GIF dépasse vite le million d'octets de Bluesky.
  `ffmpeg -i in.mov -vf "fps=12,scale=1200:-1" out.gif` et
  `ffmpeg -i in.mov -vf "scale=1200:-2" -an out.mp4`.
- **Anonymiser avant de capturer** : pas de jeton, pas de nom d'hôte
  maison, pas de chemin `/Users/<toi>`, pas d'onglet perso, prompt shell
  réduit à `$`. Fond d'écran neutre pour les captures Windows.
- **Alt obligatoire**, une phrase qui dit ce qu'on voit.
- Les captures de CTR peuvent se faire depuis le navigateur agent (skill
  `agent-browser`) sans toucher ton écran ; celles de PurrPause et du
  wizard se font sur ta machine.

## Dev Sandbox

| # | Capture | Format | Sert à |
|---|---|---|---|
| 1 | Deux `curl` côte à côte : un hôte hors allowlist refusé au DNS, un hôte autorisé qui répond | PNG | la promesse en une image |
| 2 | Le wizard `npx @meitogi/devcontainer-cli init` du début à la fin, puis « Reopen in Container » | GIF 15 s + MP4 | démo principale, README, Show HN |
| 3 | Le panneau boot-summary au démarrage du container | PNG | « ce avec quoi le container démarre » |
| 4 | Les suites `privilege.sh` et `escalation.sh` qui passent au vert dans l'image publiée | GIF 10 s | l'argument le plus défendable |
| 5 | VS Code : Claude Code dans le container à gauche, le journal des blocages firewall à droite | PNG | conseil, sécurité |
| 6 | Un hôte ajouté dans `domains.local.txt` puis `wtf firewall reload` et le diff affiché | GIF 10 s | conseil, « sans rebuild » |
| 7 | Claude qui tente un appel réseau et se fait refuser, transcript réel | PNG | humour |
| 8 | Une ligne de `policy.d/<host>.yaml` et le 403 mitmproxy correspondant en mode strict | PNG | conseil, L7 |

## CTR Native Web

| # | Capture | Format | Sert à |
|---|---|---|---|
| 9 | 10 s de course dans le navigateur, barre d'adresse visible | MP4 + recadrage 9:16 | preuve que c'est un navigateur ; Short, TikTok |
| 10 | Glisser-déposer du `.bin` dans la page jusqu'au menu du jeu | GIF 15 s | « bring your own disc », onboarding |
| 11 | Écran partagé à 4 joueurs | PNG | couch mode |
| 12 | Deux fenêtres côte à côte en Online, lobby puis départ | GIF 15 s | netplay, Show HN |
| 13 | L'overlay manette connectée | PNG | Gamepad API |
| 14 | Le mode Share : la fenêtre invitée qui reçoit le flux vidéo | GIF 10 s | « no disc needed for guests » |
| 15 | La page sur un téléphone, si ça tourne | PNG | portée mobile |

## PurrPause

| # | Capture | Format | Sert à |
|---|---|---|---|
| 16 | Le chat qui danse en plein écran par-dessus un bureau neutre | GIF 5 s + PNG | le produit en une image |
| 17 | Le widget de compte à rebours à T-5, puis T-1 | PNG ×2 | « tu as été prévenu » |
| 18 | L'écran de configuration, intervalle et code parent masqué | PNG | réglages, LinkedIn FR |
| 19 | Le zip extrait et le premier lancement avec l'UAC | GIF 10 s | installation |

Ne pas capturer : l'entrée dans `services.msc`, le nom du service, le
Gestionnaire des tâches. C'est l'angle qu'on ne vend pas.

## Transversal

| # | Capture | Format | Sert à |
|---|---|---|---|
| 20 | Un moment Claude Code réel, drôle ou impressionnant, par semaine | PNG | humour, IA |
| 21 | Extraits de code en PNG via silicon ou capture VS Code | PNG | conseils |
| 22 | Avatar et bannière rendus par `brand.mjs` | PNG | tous les profils |
| 23 | Un GIF de 5 s par produit pour le profil README `meitogi/meitogi` | GIF | GitHub |

## Premier lot de six posts et leurs captures

1. Dev Sandbox, montrer le travail : capture 1.
2. CTR, montrer le travail : capture 9.
3. Dev Sandbox, conseil : capture 6.
4. Humour : capture 7.
5. PurrPause, montrer le travail : capture 16.
6. CTR, conseil : capture 10.
