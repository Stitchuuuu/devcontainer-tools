# Meitogi — communication sur les réseaux sociaux

> Dossier de conception, 2026-10-22. Objectif : faire connaître le compte
> Meitogi (github.com/meitogi) avec un budget de **0 €** et le **moins de
> temps possible**, en pilotant tout depuis **une seule page web codée ici**.

## En bref

- **Deux réseaux publiés automatiquement** : Bluesky (principal) et Mastodon
  (miroir avec hashtags). LinkedIn en post natif hebdomadaire, collé à la
  main depuis la page. Reddit, Hacker News et Discord en événementiel.
- **X est écarté** : plus aucune API gratuite depuis février 2026, liens
  pénalisés, statistiques payantes. Hashnode aussi (API Pro depuis juin
  2026). Reddit via API aussi (approbation manuelle, petits projets refusés).
- **Un seul outil, le nôtre** : un repo GitHub public `meitogi/megaphone`
  (nom proposé) dont la **file de posts est le repo**, la **page statique
  est l'interface** (GitHub Pages ou en local), **GitHub Actions est le
  planificateur**, et **Claude est le rédacteur** via un prompt dédié.
- **Rythme** : 2 posts par semaine sur Bluesky + Mastodon, 1 sur LinkedIn,
  un événement de lancement par trimestre au plus. Deux heures par semaine.
- **Firewall déjà mis à jour** : couche committée `domains.d/social.txt` +
  miroir local hot-reloadable (`wtf firewall reload` côté host).

## Fichiers

| Fichier | Contenu |
|---|---|
| [strategie.md](strategie.md) | Positionnement, réseaux, piliers, cadence, séquence de lancement, garde-fous par produit, mesure |
| [preparation.md](preparation.md) | Checklist des comptes à créer, du paramétrage, des secrets et des soumissions one-shot |
| [outil.md](outil.md) | Architecture de la page unique, formats, flux, commandes, analyse des domaines et du firewall |
| [prompt-posts.md](prompt-posts.md) | Le prompt de mise à jour pour générer un lot de posts avec Claude |
| [captures.md](captures.md) | Les captures et GIF à faire, par produit, avec formats et règles |
| [backlog.md](backlog.md) | Le stock de départ : une quarantaine d'idées de posts par produit et pilier, et la part réponses |

## Suite

La construction de l'outil tient en 8 sessions (voir la fin de
[outil.md](outil.md)) : à scaffolder avec `/prepare-plan`.
