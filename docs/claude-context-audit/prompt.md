Fais l'audit du contexte Claude Code de ce projet avec le kit dans claude-context-audit/. Lis d'abord l'exemple claude-context-audit/exemple/rapport.md : ton rapport doit avoir exactement la même forme. Les options du script sont décrites en tête de claude-context-audit/audit.mjs.

1. Choisis le nom d'usage du projet, tel qu'on l'écrit dans une phrase (par exemple « DevContainer Tools » plutôt que « devcontainer-tools »). Puis lance, depuis la racine du projet :
   node claude-context-audit/audit.mjs --name="<nom du projet>"
   Si le script ne trouve pas le dossier des transcripts, il liste ceux qui existent : relance-le avec le bon dossier en dernier argument.

2. Le script écrit dans claude-context-audit/<nom-du-projet>/ : donnees.md, context-table.excalidraw et context-chart.excalidraw. Exporte les deux .excalidraw avec le script d'export du skill /diagram :
   - <nom>.png : rendu dark par défaut, renomme le @2x.png en <nom>.png ;
   - <nom>-dark.svg : rendu dark ;
   - <nom>.svg : rendu clair (--light --svg-only).
   Ouvre les deux PNG et vérifie qu'aucun texte ne déborde ni ne se chevauche. Si l'export n'est pas disponible, dis-le et laisse les .excalidraw.

3. Écris claude-context-audit/<nom-du-projet>/rapport.md sur le modèle de l'exemple, sections dans le même ordre :
   - titre et phrase d'intro avec le nom du projet, le nombre de sessions et la période (dans donnees.md) ;
   - le diagramme en <picture> (SVG clair et sombre), puis les liens vers les PNG et les .excalidraw ;
   - « Ce qu'on mesure » : reprends le texte de l'exemple ;
   - « Les chiffres » : le tableau de donnees.md tel quel, puis les seuils ;
   - « Les 10 sessions au plus gros contexte max » : depuis le classement de donnees.md, sans identifiant de session. Réécris chaque sujet en quelques mots génériques, sans nom de client, de personne ni de produit interne ;
   - « Ce que ça montre » : des constats propres à ce projet, classés par gain estimé. Chacun s'appuie sur un chiffre de donnees.md. Pour les 3 plus grosses sessions, ouvre le transcript et dis ce qui a fait grossir le contexte : sujets enchaînés sans /clear, pas de /compact au-delà de 200 k, gros fichiers ou sorties d'outils lus dans le fil principal, exploration qui aurait dû aller dans un sous-agent ;
   - « Méthode » : reprends le texte de l'exemple.
   Aucun cumul de tokens : on ne parle que du contexte par appel.

Ne modifie rien dans le projet en dehors de claude-context-audit/<nom-du-projet>/.
