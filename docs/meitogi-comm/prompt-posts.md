# Prompt de mise à jour — un lot de posts

À coller dans Claude Code, ouvert sur `apps/megaphone`. Session 4 de
l'outil le transforme en skill locale `/posts`. Remplacer la première
ligne par le besoin du jour.

---

Prépare un lot de posts pour Meitogi. Sujet ou période : **<semaine du
2026-11-03 | release Dev Sandbox 1.14 | CTR online fix | libre>**. Nombre :
**<3>**.

Avant d'écrire, lis dans l'ordre :

1. `../../docs/meitogi-comm/strategie.md`, sections 3 (piliers et ratio),
   6 (garde-fous) et 7 (langue). Elles priment sur tout ce qui suit.
2. `ideas.md` : prends d'abord les idées les plus anciennes non barrées.
3. `published/` : les 10 derniers posts, pour ne pas répéter un angle ni
   un format.
4. Les 14 derniers jours de commits des repos publics, via
   `gh api repos/meitogi/<repo>/commits` pour devcontainer-sandbox,
   devcontainer-cli, ctr-native-web, PurrPause. Un commit visible par
   l'utilisateur vaut un post « montrer le travail ».

Puis, pour chaque post, crée `posts/<date>-<slug>.md` au format de
`outil.md` avec `status: draft`, et :

- Respecte le ratio sur le lot et sur les 10 derniers publiés : 40 %
  travail, 30 % conseil, 20 % humour, 10 % produit. Un humour au plus par
  lot, jamais deux posts produit à la suite.
- Anglais. Bluesky 300 caractères max, texte nu, un lien au plus. Mastodon
  reçoit le même texte plus 3 à 5 `tags`. Si LinkedIn est dans `networks`,
  ajoute une variante `## linkedin` de 600 à 1200 caractères, sans lien
  dans le corps, en français si le sujet est PurrPause.
- Une seule affirmation par post, vérifiable sur GitHub. Pas de
  superlatif, pas d'emoji en rafale, pas de hashtag dans le texte Bluesky.
- `alt` obligatoire dès qu'il y a une image. Pour une carte, propose un
  titre de 6 mots max et lance `npm run card -- "<titre>" img/<slug>.png`.
  Pour un meme, construis l'URL memegen et mets-la dans `image`.
- Garde-fous : PurrPause ne se décrit jamais par la discrétion ; CTR
  rappelle « bring your own disc » dès qu'on montre du jeu ; Dev Sandbox
  ne dit rien que les tests de l'image ne prouvent.
- Dates : étale le lot sur les créneaux 09:00 et 18:00 Europe/Paris, mardi
  à jeudi, en sautant les jours où `posts/` a déjà un `ready`.

Termine par : `git add posts && git commit`, un message qui liste les
slugs ; puis affiche un tableau slug / pilier / réseaux / date / image, et
raye dans `ideas.md` les idées consommées. Ne publie rien : la relecture
se fait dans la page.
