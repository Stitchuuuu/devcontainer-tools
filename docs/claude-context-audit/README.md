# Audit du contexte Claude Code

Ce kit mesure, pour un projet, combien de contexte Claude Code relit à chaque appel. Il produit le même rapport, le même tableau et le même diagramme que [l'exemple DevContainer Tools](exemple/rapport.md).

## Utilisation

1. Copie ce dossier `claude-context-audit/` à la racine du projet à auditer.
2. Ouvre Claude Code dans ce projet et colle le contenu de [prompt.md](prompt.md), ou de [prompt.en.md](prompt.en.md) pour un rapport en anglais.

Le rapport sort dans `claude-context-audit/<nom-du-projet>/`. Rien d'autre n'est modifié dans le projet.

## Contenu

- [audit.mjs](audit.mjs) : lit les transcripts du projet et écrit `donnees.md` (tableau, seuils, classement des sessions) avec deux fichiers Excalidraw, le tableau et le diagramme.
- [prompt.md](prompt.md) et [prompt.en.md](prompt.en.md) : le prompt à coller, en français et en anglais. Claude lance le script, exporte les images en dark et écrit `rapport.md`.
- [exemple/](exemple/) : le résultat sur DevContainer Tools, qui sert de modèle.

## Le script seul

`audit.mjs` n'a aucune dépendance (Node 18 ou plus) et ne fait que lire les transcripts. On peut le lancer sans Claude, depuis la racine du projet :

```sh
node claude-context-audit/audit.mjs --name="Mon projet"
```

Options :

- `--name` : le nom affiché. Par défaut, le premier titre du README.md, sinon le `name` du package.json, sinon le nom du dossier.
- `--out` : le dossier de sortie. Par défaut, `claude-context-audit/<nom-du-projet>/`.
- un chemin en dernier argument : le dossier des transcripts. Par défaut, `~/.claude/projects/<slug du dossier courant>/`. S'il est introuvable, le script liste ceux qui existent.

Il affiche `donnees.md` et l'écrit avec les deux `.excalidraw`. Les images et `rapport.md` sont l'affaire de Claude, via le prompt.

## Ce qui est mesuré

Le contexte d'un appel, c'est ce que le modèle relit à cet appel : entrée + lecture de cache + écriture de cache. Ce n'est pas un cumul. Chaque réponse compte une fois et les sous-agents sont inclus. Le détail est dans la section « Méthode » de [l'exemple](exemple/rapport.md).
