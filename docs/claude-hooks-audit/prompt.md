# Transformer les corrections en hooks — prompt de session

Colle tout ce qui suit la ligne dans une session Claude Code ouverte à la
racine du projet à traiter.

---

Tu vas faire une passe « corrections → hooks » sur ce projet. Le but : trouver
les erreurs que j'ai déjà dû te corriger, et transformer en hooks Claude Code
celles qu'une machine peut vérifier, pour qu'elles ne passent plus. Les autres
restent des règles en texte.

Lis d'abord le CLAUDE.md du projet et suis ses règles (plan, commits, style).
En cas de conflit avec ce prompt, ce sont les règles du projet qui l'emportent.

Tout le travail se fait dans `claude-hooks-audit/` à la racine du projet. Tu
ne modifies rien d'autre avant le point de validation 3.

## Phase 1 — Lister les corrections

Sources, de la plus fiable à la plus bruitée :

1. **Règles déjà écrites** : CLAUDE.md (et les fichiers qu'il référence),
   tout fichier de leçons (`LESSONS*.md` ou équivalent), et la mémoire
   `~/.claude/projects/<slug>/memory/` (fichiers de type `feedback`).
   `<slug>` est le chemin absolu du projet où chaque `/` devient `-`.
2. **Retours de hooks existants** : dans les transcripts, les messages
   utilisateur qui commencent par `Stop hook feedback:` ou qui viennent d'un
   hook PreToolUse bloquant.
3. **Transcripts** : `~/.claude/projects/<slug>/*.jsonl`, une ligne JSON par
   événement. Les messages que j'ai tapés ont `type: "user"`, ne sont pas
   `isMeta` ni `isSidechain`, et leur `message.content` est une chaîne, ou un
   tableau de blocs `text` (pas `tool_result`). Écarte les blocs
   `<system-reminder>`, `<command-name>` et les sorties de hooks.
   **Vérifie ce format sur un vrai fichier avant de t'y fier** : il change
   d'une version de Claude Code à l'autre.
4. **Historique git** : les commits `fix`, `fixup`, `revert`, et
   `git log -S` sur les motifs suspects. Lis l'historique entier, pas
   seulement les derniers commits.

Méthode :

- Écris un petit script Node sans dépendance qui extrait mes messages des
  transcripts dans `claude-hooks-audit/messages.jsonl`. Pour chaque message,
  garde : le fichier, l'horodatage, le texte, et le dernier message de
  l'assistant juste avant. Ne lis pas les transcripts entiers dans ta session
  principale.
- Lance des **sous-agents en parallèle**, un par lot de messages, pour
  repérer les corrections. Une correction, c'est :
  - un refus (« non », « pas ça », « je t'ai dit ») ;
  - une consigne répétée ;
  - une demande de refaire ;
  - un retour de hook.

  Chaque sous-agent renvoie des lignes structurées, pas un résumé.
- Fusionne les corrections qui disent la même chose, puis compte leurs
  occurrences.

Livrable : `claude-hooks-audit/corrections.md`, un tableau avec une ligne par
correction :

| Règle | Source(s) | Occurrences | Exemple fautif réel | Exemple correct réel |
|---|---|---|---|---|

Les exemples sont des **extraits réels**, cités avec leur fichier et leur date.
Jamais des exemples inventés.

**Point de validation 1** : montre-moi le tableau et attends ma réponse. Je
retire ce qui est faux ou hors sujet.

## Phase 2 — Trier : une machine peut-elle trancher ?

Pour chaque règle retenue, réponds dans l'ordre et arrête-toi au premier oui :

1. **Un outil du projet le vérifie-t-il déjà** (linter, formateur,
   shellcheck, test, hook git) ? → verdict « brancher l'outil », dans un hook
   PostToolUse qui le lance sur le fichier modifié.
2. **Une expression régulière, un parseur ou un code de retour suffit-il à
   trancher sans jugement ?** → verdict « hook ».
3. **Est-ce une procédure en plusieurs étapes ?** → verdict « skill ».
4. Sinon → verdict « leçon » : la règle reste en texte.

Pour chaque « hook », choisis le moment :

| Moment | Contrôle | Exemple |
|---|---|---|
| PreToolUse, outil `Bash` | une commande avant son exécution | bloquer une commande destructive |
| PreToolUse, outils `Write` / `Edit` | un contenu avant son écriture | refuser un chemin absolu dans un script destiné à l'hôte |
| PostToolUse | le résultat, après coup | lancer le linter sur le fichier écrit |
| Stop | ma réponse finale | refuser un emoji ou un lien mal formé |

Écarte d'office :

- les règles vues une seule fois ;
- les règles de jugement (« demander avant de refaire un design ») ;
- tout hook qui ferait doublon avec un outil existant.

Livrable : `claude-hooks-audit/tri.md`, qui reprend le tableau avec les
colonnes Verdict, Moment, Contrôle prévu et Raison.

**Point de validation 2** : montre-moi le tri et attends ma réponse.

## Phase 3 — Écrire et prouver chaque hook

**Avant d'écrire le premier hook, lis la documentation officielle des hooks
Claude Code** (code.claude.com, page Hooks). Le format d'entrée et de sortie
vient de là, pas de ta mémoire. Points à confirmer dans la doc :

- ce que le hook reçoit sur stdin ;
- comment il bloque (code de sortie, ou JSON avec décision et raison) ;
- le champ qui signale qu'un hook Stop a déjà bloqué une fois. Un hook Stop
  qui l'ignore fait tourner la session en boucle.

Contraintes pour chaque hook :

- Node ou Bash, **sans dépendance**, moins de 100 ms : il tourne à chaque
  appel d'outil.
- Son message dit **quoi corriger et comment**, dans la langue dans laquelle
  je t'écris.
- Un hook par fichier, rangé dans `claude-hooks-audit/hooks/`.

Pour chaque hook, **avant de proposer de l'installer** :

1. **Tests** : un fichier de cas avec au moins les exemples fautifs et
   corrects réels de la phase 1. Le hook doit bloquer tous les fautifs et
   laisser passer tous les corrects. Lance les tests et colle leur sortie.
2. **Rejeu** : rejoue le hook sur l'historique des transcripts :
   - un hook PreToolUse sur les `tool_use` passés (commandes Bash, contenus
     écrits) ;
   - un hook Stop sur les réponses finales passées.

   Compte ce qu'il aurait bloqué. Relis chaque blocage et classe-le : **vraie
   faute** ou **faux positif**.
3. **Seuil** : garde le hook seulement s'il attrape au moins une vraie faute
   et ne produit **aucun** faux positif. Sinon, resserre le motif et rejoue,
   ou repasse la règle en « leçon ».

Livrable : `claude-hooks-audit/rapport.md`. Pour chaque hook :

- la règle ;
- le moment ;
- le résultat des tests ;
- le rejeu (vraies fautes / faux positifs) ;
- le verdict.

Ajoute-y la liste des skills candidats et des leçons restantes.

**Point de validation 3** : montre-moi le rapport et attends ma réponse avant
toute installation.

## Phase 4 — Installer, après mon accord

1. **Choisis l'emplacement selon le projet**, et dis-moi lequel tu retiens :
   - si le projet a déjà un mécanisme de hooks par skill (un dossier de skills
     avec des `hooks.json` fusionnés au démarrage), suis-le ;
   - sinon, mets les scripts dans `.claude/hooks/` et déclare-les dans
     `.claude/settings.local.json`, personnel et non commité. Utilise
     `.claude/settings.json` si je veux les partager.
2. **Déclenche chaque hook une fois en vrai** dans la session, pour montrer
   qu'il bloque.
3. **Propose le retrait des règles devenues hooks** : sous forme de diff sur
   le CLAUDE.md ou le fichier de leçons, sans l'appliquer. Une règle tenue par
   un hook n'a plus besoin d'être relue à chaque session.
4. **Commite** selon les règles de commit du projet. Le message décrit les
   hooks ajoutés et ce qu'ils empêchent, sans référence à ce prompt.

## Ce que tu ne fais pas

- Modifier un fichier hors de `claude-hooks-audit/` avant le point de
  validation 3.
- Inventer un exemple de faute : seuls des extraits réels servent de preuve.
- Installer un hook qui n'a pas passé ses tests et son rejeu.
- Créer un skill : tu le proposes seulement, dans le rapport.
