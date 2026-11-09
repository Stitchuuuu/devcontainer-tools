# Veille nxnative : Pokopia, Hyrule Warriors SW2, Ocarina of Time SW2

Notes de mise à jour des trois projets suivis, pour trois jeux Switch 2 :
Pokémon Pokopia (`0400A3A01EE4C000`), Hyrule Warriors: Age of Imprisonment
(`04003F40232DE000`) et The Legend of Zelda: Ocarina of Time (`0400486024DEC000`).
Point de départ : vendredi 6 novembre 2026 (installation). Chaque passage ajoute
une section datée en tête du journal et met à jour le point de reprise.
Le prompt à relancer est [PROMPT.md](PROMPT.md).

| Projet | Où | Rôle |
|---|---|---|
| nxnative | [github.com/seedds/nxnative](https://github.com/seedds/nxnative) | le moteur : Switch / Switch 2 natif sur Mac, iPhone, iPad |
| NxNative Launcher | [gitlab.com/rennan-group2/nxnative_Launcher](https://gitlab.com/rennan-group2/nxnative_Launcher) | front end Mac de nxnative (l'ancienne adresse GitHub `rennan101/nxnative_Launcher` répond 404) |
| pokopia-native | [github.com/xFractals/pokopia-native](https://github.com/xFractals/pokopia-native) | Pokopia sur Windows, Mac, iPhone, Linux, Steam Deck, Android ; patch Switch 2 sur nxnative |

## Point de reprise

Le prochain passage lit tout ce qui est **après** ces repères.

| Projet | Dernier commit lu | Dernière version lue | Lu le |
|---|---|---|---|
| nxnative | `04811a63` (2026-11-10) | — (pas de releases, on suit `main`) | 2026-11-09 |
| NxNative Launcher | `d6182890` (2026-11-09) | tag `v2.0.1` | 2026-11-09 |
| pokopia-native | `8618190` sur `main` (2026-11-09) | `v1.4.0`, `v1.3.3-android-beta.2`, `v1.3.3-linux-beta.5` | 2026-11-09 |

Base du patch Switch 2 de pokopia-native : nxnative `c1df4dd` (2026-11-01), voir
`apps/pokopia-native/host/UPSTREAM.md`.

---

## 2026-11-09 — du vendredi 6 au lundi 9 novembre

### nxnative (205 commits sur `main` depuis le 6/11)

**Pokopia** : aucun commit dédié depuis le 6/11. Pour contexte, le support est
arrivé juste avant : `03d92a9` « Pokémon Pokopia plays on the Mac » (3/11), la
bibliothèque partielle de 8 158 shaders et l'option `--seen-shaders` (4/11), le
builder deux fois moins gourmand en CPU (5/11). État de sa fiche : intro, choix du
personnage et tutoriel de la première zone à ~60 fps, 31 min sans erreur GPU,
sauvegarde rechargée ; pas d'IPA ; pas encore de `pso_manifest.tsv`.

**Hyrule Warriors: Age of Imprisonment** : aucun commit dédié depuis le 6/11.
Contexte : premier terrain atteint le 3/11 (16,6 ms médian), deux derniers
shaders non traduits corrigés le 4/11. Combats pas atteints, pas d'IPA.
À ne pas confondre avec *Age of Calamity* (Switch 1) : 60 fps en v458752 et
cinématiques à 30 fps (`143c0d09`, `922679a1`).

**Ocarina of Time (Switch 2)** — le jeu le plus travaillé de la période, 58 commits :
- `b1d7c707` (6/11) : jouable sur Mac à ~59 fps jusqu'à la forêt Kokiri, image
  proche de la vidéo console. Mac seulement.
- Rendu : feuillages (mesh shaders) qui flashaient en blanc ou ne se
  construisaient pas, taches blanches de la forêt Kokiri, trous bleus et tuiles
  de terrain qui disparaissaient, brume qui scintillait, pointillés dans le
  brouillard, bande claire dans la salle de Gohma, tirets bleus à l'entrée de la
  grotte de la Montagne de la Mort, peintures des sœurs Poe et de Ganon spectral.
- Crashs : à la première note d'ocarina, au chargement de la forêt Kokiri
  (AGX), au chargement d'une sauvegarde avec le thread d'encodage, défauts de
  page GPU après 15 à 45 min de jeu ou sur M2 Max.
- Performances : pleine résolution et résolution dynamique qui remonte jusqu'à
  la taille max ; plus de ralenti quand une frame est longue (pas de frame de
  l'hôte) ; premier démarrage 713 s → 23,6 s puis 3,3 s (archive de pipelines) ;
  pipelines task/mesh préchauffés (fini les 59 s d'arrêt à l'entrée de Kokiri) ;
  vidéo de titre 12,8 → 6,8 ms GPU (réseau NDVE exécuté par l'hôte) ; route du
  pont Kokiri 54 → 59 fps sur M4 Pro ; ProMotion en plein écran tient 60 fps
  (40 avant) ; empreinte mémoire 19,0 → 12,2 Go sur disque externe.
- iPad : premiers essais (un iPad s'est arrêté faute de place disque, 5 Go
  libres pour 6,6 Go de fichiers ; corrigé par `8d5d4f1b`).
- Construction : un NUP endommagé arrête la build ; une build via NX2Apple qui
  donnait un écran noir avec le son est maintenant refusée proprement (issue
  #1841).

**Switch 2 en général** : Joy-Con 2 et Pro Controller 2 en Bluetooth LE sur Mac
(réglage désactivé par défaut, vibration, gyroscope), le README liste 11 jeux
Switch 2 testés sur Mac dont les trois suivis.

### NxNative Launcher (v1.9.6 → v2.0.1)

- **v1.9.6** (6/11) : portage natif Mac d'Ocarina of Time (Switch 2), `zbic`
  embarqué, galerie de 144+ jeux ; exigences corrigées : macOS 26.6+, iOS 26.0+,
  Xcode 27.2+, Metal 4.
- **v1.9.7** (6/11) : Ocarina à 60 fps en 3D, correctif d'éclairage (atomiques
  flottants), file de compilation Metal 4 sérialisée.
- **v1.9.8** (6/11) : correctifs de portage (nettoyage après build qui
  échouait sur `.DS_Store`, garde sur les shaders non-vertex).
- **v1.9.9** (7–8/11) : section **Mises à jour** dans les réglages ; pour
  Ocarina : overlay RomFS et réglages générés automatiquement, lecteur vidéo à
  60 fps, chaîne MetalFX, optimisations pour Mac 16 Go, fin des textures roses
  et noires ; synchronisations avec l'amont ; modèles d'issues GitLab.
- **v2.0.0** (8/11) : nouvelle interface « Liquid Glass » (fond plein écran,
  dock de navigation à gauche, bibliothèque unifiée, onglets Switch 1 & 2 /
  iOS & iPadOS / non portés), mise à jour proposée au démarrage, jaquettes
  Switch 2 ; pour Ocarina : résolution fixe, netteté CAS, contournement de
  l'upscaler, réglages appliqués à chaud ; synchro amont (Metal 4).
- **v2.0.1** (9/11) : interrupteur par jeu pour couper le remappage manette
  (entrées passées telles quelles) ; Ocarina démarre avec des réglages par
  défaut : 60 fps, mode jeu macOS, CAS 35 %, résolution dynamique, micro activé
  (chants d'ocarina), cinématiques à 60 fps en contournant l'upscaler NDVE.
  Téléchargement : `NxNative_Launcher.dmg` (68 Mo) sur la
  [page de la release](https://gitlab.com/rennan-group2/nxnative_Launcher/-/releases/v2.0.1).
- Après v2.0.1 : `d6182890`, le vérificateur de mises à jour met en cache
  l'API GitLab pour ne plus être limité.

Pokopia et Age of Imprisonment : rien de dédié dans le Launcher ; ils profitent
du support Switch 2 général (build depuis `.nup` ou dossier de dump, jaquettes,
lancement depuis la barre des tâches) et des synchros avec l'amont.

### pokopia-native (v1.2.6 → v1.4.0, Android beta 2, Linux beta 5)

- **v1.2.6** (6/11) : les effets de particules sautés en 1.2.4 sont dessinés ;
  lanceur Pokopia Settings sur Windows ; dossier avec crochets dans le nom
  corrigé.
- **Linux / Steam Deck, première bêta ouverte** (6/11, sur 1.2.6) : testée
  seulement sous WSL avec rendu logiciel.
- **v1.3.0** (6/11) : sur Windows, résolution réelle jusqu'à 4K, réglages dans
  le jeu (F1, ou − et + ensemble), un tiers de CPU en moins par frame (merci
  Quantumenis), réglage 30 fps réparé ; builder pas à pas sur Windows et Mac ;
  Story co-op (expérimental).
- **Linux bêtas 2 et 3** (7/11) : démarre sur Fedora, Arch et le Steam Deck
  (#30) ; son sans craquements sous PipeWire.
- **v1.3.1** (7/11) : gel NVIDIA (GTX 1650, RTX 3050 Laptop), build dans un
  dossier accentué, .NET détecté juste après l'installation, mode test AMD/Intel.
- **v1.3.2** (7/11) : correctif du crash AMD de la 1.3.1 (le correctif NVIDIA
  ne s'applique plus qu'aux cartes NVIDIA).
- **v1.3.3** (7/11) : Interact everywhere et Story co-op aussi sur le jeu en
  1.0.5 ; caméra souris qui ne tournait plus en marchant de côté (#31) ; autre
  gel NVIDIA ; ralentissements près de l'eau (#27).
- **Linux bêta 4** (7/11) : la 1.3.3 sous Linux, 5 fois moins de données
  envoyées au GPU par frame.
- **v1.3.4** (7/11) : Wide view pour écrans 21:9 et plus (expérimental), caméra
  souris à pleine vitesse en strafe, message clair quand aucune carte graphique
  utilisable n'est trouvée.
- **Linux bêta 5** (8/11) : plus d'étincelles sur écrans 16:10, fréquence plus
  stable sur GPU intégré, builder bien affiché, outil de rapport qui trouve la
  build seul.
- **v1.3.5** (8/11) : remappage des boutons manette et choix du joueur 1 ;
  beaucoup plus de manettes (Steam Controller, base SDL_GameControllerDB) ;
  onglet Link Play dans le jeu (adresse, code de salon, invitation à copier) ;
  ouverture de port automatique (UPnP, NAT-PMP, PCP) quand on héberge ; gels
  NVIDIA et AMD ; caméra plus douce en strafe (aussi sur Mac et iPhone).
- **Android bêta 2** (9/11) : première bêta Android publique. L'app est
  construite sur un PC Windows et installée par câble USB ; un APK de test dit en
  quelques secondes si le téléphone peut faire tourner le jeu. Runtime 1.3.3,
  builder 1.4.0.
- **v1.4.0** (9/11) : DLC (votre propre dump) sur Windows et Linux, Mac et
  iPhone plus tard ; Linux et Steam Deck sortent de bêta (AppImage, mode
  portable sur Deck) ; anti-aliasing SMAA (défaut) ou FXAA ; co-op en écran
  partagé sur un seul PC (expérimental, Windows) ; `Play Pokopia <version>.exe`
  à épingler ou ajouter à Steam ; Story co-op réparé côté hôte ; rendu sur son
  propre thread (+16 % d'images quand le CPU limite).
- Après v1.4.0 : `8618190` sur `main` (9/11, pas encore publié), quatre formes
  d'instructions Ampere de plus traduites pour Pokopia.

### Projets dédiés Windows / Mac

Trouvés par recherche web le 9/11. Aucun de ces dépôts n'a été lu ; les infos
viennent de la presse ou d'agrégateurs.

- **Pokopia, Windows** : [pokopia-recomp (drakolordx7)](https://hotrepo.ai/drakolordx7/pokopia-recomp),
  recompilation statique avec rendu Vulkan, pour la version 2.0.1a seulement,
  cache de shaders de 1 à 2 h ; fork avec multijoueur
  [Recompia](https://github.com/featureJosh/Recompia). Les sources ne
  s'accordent pas sur le dépôt ni sur la version à jour.
- **Ocarina SW2, Windows** :
  [Hookshot (NotRexed)](https://www.techeblog.com/hookshot-switch-2-ocarina-of-time-remake-pc-port-recomp/),
  le plus visible mais qui couvre encore peu du jeu ;
  [ocarina-recomp](https://github.com/sostenesfreitas/ocarina-recomp) ;
  [Echora](https://github.com/Mars7x/echora), en Rust et pas encore jouable ;
  [PC Native Build](https://ocarina-pc-native-build.github.io/), sans code ni
  release.
- **Age of Imprisonment, Windows** : [hwaoi-recomp](https://github.com/Alano6678/hwaoi-recomp),
  première scène 3D affichée, gameplay non vérifié, code de recherche.
- **Mac** : aucune recomp dédiée trouvée pour ces trois jeux ; la seule voie
  est nxnative et ses front ends (NxNative Launcher, NX2Apple, NX Play, LumaNX).
