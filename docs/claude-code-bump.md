# Prompt — passer à une nouvelle version de Claude Code (extension VS Code)

À coller dans une session Claude Code ouverte sur ce dépôt, en remplaçant
`<NEW>` (la version visée, ex. `2.1.322`) et `<OLD>` (la ligne par défaut
actuelle, ex. `2.1.310`). Si `<NEW>` n'est pas encore choisie, laisse-la vide :
l'étape 0 la propose.

---

On passe l'extension Claude Code de `<OLD>` à `<NEW>`. Trois dépôts bougent, dans
cet ordre, plus ce dogfood :

1. `packages/claude-ext-patchs` — les patchers (privé) ;
2. `packages/devcontainer-sandbox` — l'image publiée sur GHCR (public) ;
3. `packages/devcontainer-cli` — les versions par défaut du scaffold ;
4. `.devcontainer/` de ce dépôt — l'image de base épinglée et le patcher local
   `model-timing-probe`.

Lis d'abord, en entier : `packages/claude-ext-patchs/CLAUDE.md`,
`packages/claude-ext-patchs/UPGRADING.md` (la procédure de référence, ne la
recopie pas, suis-la) et `packages/devcontainer-sandbox/RELEASING.md`. Passe en
plan mode avant toute écriture, et pose-moi les décisions listées à l'étape 0.

## 0. Choisir et mesurer — avant le plan

- **Version** : `npm view @anthropic-ai/claude-code versions --json | tail` et
  `npm view @anthropic-ai/claude-code dist-tags` ; le changelog est lisible
  depuis le conteneur :
  `curl -sS https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md`.
  Cite-moi les entrées qui justifient la version (modèle ajouté, correctif).
- **Décisions à me demander** : la version exacte ; la matrice (d'habitude
  `<OLD>` + `<NEW>`, l'ancienne ligne sort — ses tags publiés restent, on ne les
  supprime jamais) ; ce qui doit se faire dans cette session ou la suivante.
- **Mesure avant de chiffrer** — sur une copie, jamais dans l'arbre :

  ```bash
  cd packages/claude-ext-patchs
  scripts/vendor-fetch.sh <NEW> linux-arm64
  ```

  Puis copie le dépôt (sans `vendor/` ni `.git/`) dans le scratchpad, lie
  `vendor/`, mets `versions.json` à `["<OLD>","<NEW>"]`, lance
  `python3 scripts/build-manifest.py` et
  `TOOLKIT_DIR=/workspace/packages/devcontainer-sandbox/assets/vscode-ext-patchs bash test/run.sh`.
  Lis **le compte** (`apply: N passed, M failed`) et la liste des `FAILED` /
  `✘`, jamais le code de sortie de `run-all.sh` (toujours 0).
- **Le patcher local** `.devcontainer/claude/vscode-ext-patchs/model-timing-probe.py`
  n'est pas dans la suite : applique-le seul avec `run-all.sh` sur une copie de
  `vendor/v<NEW>/min` (`CLAUDE_CODE_EXT_PATCHS=all`), et compte
  `grep -o 'mt-v2' extension.js | wc -l` (référence : 7) et `[model-timing]`
  (référence : 6).

## 1. Patchers — `packages/claude-ext-patchs`

1. `versions.json` (versions, `default`, commentaire daté) puis
   `python3 scripts/build-manifest.py`.
2. Pour chaque patcher en échec : applique-le seul sur une copie fraîche de
   `vendor/v<NEW>/min` pour lire sa raison (`Reason: …`), compare le bundle
   `<OLD>` et `<NEW>` autour de l'ancienne ancre, et re-ancre sur la structure
   la plus stable (`AUTHORING.md`). L'ancre doit couvrir **les deux** versions
   de la matrice. Vérifie que la sortie sur `<OLD>` ne change pas (md5 du
   fichier patché avant/après).
3. Un patcher qui dépend d'un autre (ex. `paste-model-detect` lit
   `model-selection-fix.py`) échoue par ricochet : teste-les ensemble.
4. Un test de comportement rouge sans patcher en échec est une **forme** qui a
   changé dans `test/behavior.py` — ou une régression amont. Tranche sur le
   bundle, jamais sur le changelog.
5. Un nouveau modèle : la table `RELEASE_DATES` de
   `opus-4-7-legacy-picker-fix.py` (source de la date en commentaire), et
   vérifie que le binaire le contient :
   `grep -aoE 'claude-<famille>-[0-9-]+' vendor/…/v<NEW>/min/resources/native-binary/claude | sort -u`.
6. Une section par patcher touché dans `PATCHES.md` (une phrase : ce qui a bougé
   en `<NEW>`).
7. `test/run.sh` complet, vert sur les deux versions → commit (identité
   `meitogi`, déjà configurée dans ce dépôt).
8. Tags : un par ligne de `versions.json`, **même `-r<n>`** pour toutes
   (`git tag -n1 -l 'cc*'` donne le max), annotés, `--cleanup=verbatim`, notes en
   français au format de `CLAUDE.md` (titre joueur, `## Quoi de neuf`,
   `## Technique`). `scripts/release-tags.sh -r <n> -F <notes> --dry-run`
   vérifie le message. Crée les tags en local, puis écris un lien de release
   pré-rempli par tag dans `.tmp/` (la ligne la plus récente garde « Latest »).

## 2. Image — `packages/devcontainer-sandbox`

`cc-versions.json` (versions, `default`, commentaire), `Dockerfile`
`ARG CLAUDE_CODE_VERSION`, `package.json` (bump mineur quand le défaut change),
`NEXT-UPDATE.md` (lis-le : des changements non publiés partent avec la
release). `bash test/manifest.test.sh` → commit `release: <ver> — …`. Notes de
tag en anglais (`v<ver> — …`, `## What's new`, `## Technical`, `Cuts …`) dans
un fichier sous `.tmp/` ; **pas de tag** : il vient après le gate du Mac.

## 3. Ce qui se fait sur le Mac — à me donner, pas à faire

- `scripts/bare-check.sh <NEW>` (+ `--login`, et `--login` sur `<OLD>` pour
  comparer) : un patcher **covered** par l'amont se borne
  (`@patch-max-version`), il ne se ré-ancre pas.
- `scripts/patched-check.sh <NEW> <patchers ré-ancrés>` : prouve qu'ils marchent.
- Push des tags des patchers, puis les releases via les liens pré-remplis.
- `wtf image release-check --no-purge` (`tier=GREEN_PARTIEL`, `red=0`, 9 lignes,
  `⊘` sur les étapes 5 et 9 seulement), puis
  `git tag -a v<ver> --cleanup=verbatim -F <notes>` et push : la CI publie
  `<ver>-cc<OLD>` et `<ver>-cc<NEW>`.

## 4. Après publication sur GHCR — session suivante

- `packages/devcontainer-cli/src/lib/docker.ts` : `DEFAULT_CLAUDE_CODE_VERSION`,
  `DEFAULT_BASE_VERSION`, `PUBLISHED_CLAUDE_CODE_VERSIONS`, et les tests qui les
  citent ; `npm test`. Pas avant : le scaffold pointerait une image absente.
- Dogfood : `.devcontainer/Dockerfile` et `.devcontainer/docker-compose.yml`
  (le tag `BASE_IMAGE` et le commentaire des lignes publiées), le patcher local
  ré-ancré et son `README.md`. Signale-moi `CLAUDE_CODE_VERSION` et
  `EXT_PATCHES_REF` dans `.devcontainer/.env` (fichier local, gitignoré) s'ils
  désignent une autre version.
- Je fais le Rebuild. Vérifications ensuite : `claude --version`,
  `ext-patches-sync --status` (le tag `cc<NEW>-r<n>`, tout `live`), et le
  nouveau modèle visible et cochable dans le sélecteur.

## Pièges déjà rencontrés

- **Le minifieur change de conventions** d'une version à l'autre (paramètres
  `$` puis `e`, flèches `()=>{return X}` puis `()=>X`). Une ancre qui écrit
  `\$` ou suppose un bloc casse partout à la fois : capture le nom, accepte les
  deux formes.
- **Jamais de nom court dans le code injecté.** En 2.1.322 la fabrique JSX
  s'appelait `k` et un paramètre `k` du patcher la masquait : tout appliquait,
  `node --check` passait, la pill rendait `null`. Seuls les tests de
  comportement l'ont vu.
- **« Applique » n'est pas « sert »** : un vert de `test/run.sh` ne dit rien de
  l'utilité (bare-check) ni du fonctionnement réel (patched-check).
- Le patcher local du dogfood n'est couvert par aucune suite : il se teste à
  part, sur les deux versions.
- **Un nouveau modèle absent du sélecteur, même sans patcher** : le CLI lit ses
  lignes dans le catalogue signé `downloads.claude.ai/model-catalog/v1/`, et
  retombe sinon sur la copie embarquée dans son binaire, en retard d'un modèle
  (2.1.322 : « Fable 5.1 »). Le pare-feu de base l'autorise depuis
  devcontainer-sandbox 1.18.0 ; si le modèle manque encore, regarde
  `firewall-blocks` avant de toucher un patcher.
