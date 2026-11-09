# Prompt : veille nxnative (Pokopia, Hyrule Warriors SW2, Ocarina of Time SW2)

Copier tout le bloc ci-dessous dans une nouvelle session Claude Code ouverte sur
`/workspace`.

---

Fais la veille nxnative décrite dans `docs/nxnative-veille/PROMPT.md`.

**Ce qu'on suit.** Trois jeux Switch 2 : Pokémon Pokopia (`0400A3A01EE4C000`),
Hyrule Warriors: Age of Imprisonment (`04003F40232DE000`) et The Legend of Zelda:
Ocarina of Time (`0400486024DEC000`). Dans trois projets :

- **nxnative** (github.com/seedds/nxnative), clone local
  `apps/pokopia-native/nxnative`. Pas de releases : on suit les commits de `main`.
- **NxNative Launcher** (gitlab.com/rennan-group2/nxnative_Launcher, projet
  GitLab id 88419597). L'ancienne adresse GitHub `rennan101/nxnative_Launcher`
  répond 404 : ne pas l'utiliser.
- **pokopia-native** (github.com/xFractals/pokopia-native), clone local
  `apps/pokopia-native`, branches `main`, `linux-beta` et `android-beta`.

**1. Lire le point de reprise** dans `docs/nxnative-veille/NOTES.md` (section
« Point de reprise ») : dernier commit et dernière version lus pour chaque
projet. Tout ce qui est après est nouveau.

**2. Récupérer le nouveau.**

- nxnative : dans `apps/pokopia-native/nxnative`,
  `git -c safe.directory='*' fetch origin` (fetch seulement : jamais de
  checkout ni de pull, le clone est volontairement sur `c1df4dd` avec le patch
  Switch 2 de pokopia-native appliqué). Puis
  `git -c safe.directory='*' log --format='%h %cs %s' <dernier>..origin/HEAD`.
  Garder ce qui cite un des trois jeux, son title ID, et ce qui touche le
  Switch 2 en général (NUP, NVN2, Ounce, manettes Switch 2, liste des jeux du
  README). Relire aussi les fiches des trois jeux sur `origin/HEAD`
  (`dm/games/<title id>/README.md`, premières lignes) pour noter un changement
  d'état (iPhone/iPad, IPA, zones atteintes).
- NxNative Launcher : API GitLab, sans authentification :
  `https://gitlab.com/api/v4/projects/88419597/repository/tags`,
  `.../releases` et `.../repository/commits?since=<date du point de reprise>`.
  Les notes complètes sont dans la release GitLab (souvent seulement la
  dernière) ; pour les autres tags, le message du tag et les commits
  `feat` / `fix` / `release` du Launcher lui-même. Les commits fusionnés de
  l'amont (nxnative) sont déjà couverts par la partie nxnative.
- pokopia-native : `git -c safe.directory='*' fetch origin` dans
  `apps/pokopia-native` (fetch seulement), les commits des trois branches après
  le point de reprise, et les releases :
  `https://api.github.com/repos/xFractals/pokopia-native/releases`.
- Projets dédiés : une recherche web par jeu pour les recomp ou portages
  natifs **Windows 10/11** et **Mac** (nouveaux projets, nouvelles versions des
  projets déjà listés dans la dernière section « Projets dédiés »).

**3. Écrire les notes.** Dans `docs/nxnative-veille/NOTES.md`, ajouter **en
tête du journal** (juste sous le `---` qui suit le point de reprise) une section
`## <date du jour> — du <date du point de reprise> au <date du jour>`, avec les
mêmes sous-sections que la précédente : nxnative, NxNative Launcher,
pokopia-native, Projets dédiés. En français, par version ou par thème, chaque
ligne avec sa date et son commit, sa version ou son lien. Écrire chaque note
avec tes propres mots : ne pas recopier les notes de release. Si rien n'est
nouveau pour un jeu ou un projet, l'écrire en une ligne. Les infos qui viennent
du web et pas d'un dépôt lu sont marquées comme telles. Puis mettre à jour le
tableau « Point de reprise » (commits, versions, date de lecture). Ne pas
modifier les sections des passages précédents.

**4. Signaler ce qui touche pokopia-native.** Le patch Switch 2 de pokopia-native
est fait contre nxnative `c1df4dd` (`apps/pokopia-native/host/UPSTREAM.md`).
Si l'amont change ce que le patch touche (Pokopia, `dm/host`, `dm/gfx/nvn`,
la chaîne de shaders), le dire dans la réponse.

**5. Répondre et commiter.** Dans le chat : un résumé court par jeu, puis les
trois projets, avec un lien vers `docs/nxnative-veille/NOTES.md`. Commiter
uniquement `docs/nxnative-veille/NOTES.md` dans le dépôt `/workspace`.
