# Sous-agents, skills et hooks : faire travailler Claude comme un modèle entraîné

On ne peut pas entraîner Opus ni Fable : Anthropic ne propose de fine-tuning
que pour Claude 3 Haiku, sur Amazon Bedrock. Trois outils de Claude Code
couvrent quand même ce qu'un entraînement apporterait :

| Ce qu'un entraînement apporterait | Équivalent Claude Code |
|---|---|
| Ne pas relire tout le projet à chaque tâche | **Sous-agents** : ils lisent, la session principale ne reçoit que la conclusion |
| Savoir faire une procédure sans qu'on la réexplique | **Skills** : chargés seulement quand ils servent |
| Ne plus refaire une erreur déjà corrigée | **Hooks** : une vérification automatique qui bloque l'erreur avant qu'elle passe |

Ce guide propose deux choses :

1. ajouter ces trois réflexes à `/prepare-plan` ;
2. créer, pour chaque projet, une passe qui transforme les corrections déjà
   faites en hooks.

## 1. Dans `/prepare-plan`

### Ce qui existe déjà

- Le bloc « Sub-agents » du skill. Il délègue l'exploration à Haiku, la
  vérification finale à Sonnet et l'écriture du prompt de la session suivante
  à Opus.
- La section **Gotchas** de chaque entrée de `LOG.md`, qui note les surprises
  d'une session.
- La boucle d'apprentissage du CLAUDE.md (§8). Elle écrit les corrections dans
  [LESSONS.md](../.devcontainer/LESSONS.md), dans `LESSONS.local.md` ou dans
  la mémoire.

Ce qui manque, c'est l'étape suivante. Les leçons restent du texte : Claude
les relit à chaque session et peut encore les oublier.

### Les trois ajouts proposés

**a. Une règle de délégation chiffrée, plutôt que « un maximum de sous-agents ».**
Un sous-agent n'est pas gratuit : il recharge le prompt système et le
CLAUDE.md, et sa conclusion peut perdre un détail. La règle à ajouter au bloc
« Sub-agents » :

- plus de 3 fichiers à lire pour répondre à une question → un sous-agent ;
- N unités indépendantes (un fichier, un module, une source) → N sous-agents
  en parallèle, dans un seul message ;
- la décision et la synthèse restent dans la session principale.

Le seuil de 3 fichiers est à vérifier avec le kit
[claude-context-audit](claude-context-audit/README.md), qui mesure ce que
chaque appel relit.

**b. Une vérification des répétitions, à ajouter au DoD de chaque session.**
Ajouter une ligne **Repeated** au modèle de `LOG.md` : les procédures faites à
la main pendant la session. Quand une procédure apparaît dans deux entrées du
LOG, la session suivante **propose** d'en faire un skill. Elle ne le crée pas
d'elle-même, comme le veut la règle des signaux du CLAUDE.md (§11).

**c. Le tri des Gotchas.**
À la fin de la session, chaque Gotcha reçoit une étiquette :

- `hook?` : vérifiable par une machine ;
- `skill?` : une procédure à réutiliser ;
- `leçon` : demande du jugement.

La passe décrite ci-dessous traite les `hook?`.

## 2. La passe « corrections → hooks », générique par projet

### Pourquoi un skill plutôt qu'un prompt à coller

La même démarche sert dans chaque projet. D'après la règle 1b, c'est un skill.
Il peut vivre dans l'image de base, à côté de `prepare-plan`, et livrer son
propre `hooks.json`, que `sync-skills` fusionne au démarrage du conteneur. Un
prompt à coller marche aussi pour un premier essai : c'est
[claude-hooks-audit/prompt.md](claude-hooks-audit/prompt.md).

### Étape 1 — Lister les corrections

Les sources, de la plus fiable à la plus bruitée :

| Source | Ce qu'on y trouve |
|---|---|
| `LESSONS.md` / `LESSONS.local.md` | Les règles déjà formulées |
| Mémoire, fichiers de type `feedback` | Les préférences transverses |
| Retours des hooks Stop existants | Les erreurs déjà bloquées une fois, avec leur fréquence |
| Transcripts `~/.claude/projects/<projet>/*.jsonl` | Les messages où tu corriges Claude, surtout ceux où tu répètes une consigne déjà donnée |
| `git log` (commits `fix`, `fixup`, `revert`) | Les erreurs qui sont arrivées jusqu'au commit |

Le script `audit.mjs` du kit de contexte lit déjà ces transcripts. On peut
réutiliser son analyseur.

Chaque correction devient une ligne : **règle**, **exemple réel fautif**,
**exemple réel correct**, **nombre d'occurrences**.

### Étape 2 — Trier : la machine peut-elle trancher ?

| Question | Si oui | Si non |
|---|---|---|
| Un outil existant le vérifie-t-il déjà (eslint, prettier, shellcheck, un test) ? | Brancher cet outil dans un hook PostToolUse | Question suivante |
| Une expression régulière ou un code de retour suffit-il à trancher ? | **Hook** | Question suivante |
| Est-ce une procédure en plusieurs étapes ? | **Skill** | Reste une **leçon** en texte |

Ensuite, choisir le moment du hook :

| Moment | Ce qu'il contrôle | Exemple |
|---|---|---|
| **PreToolUse** sur Bash | Une commande avant son exécution | Bloquer `docker compose … down -v` |
| **PreToolUse** sur Write/Edit | Un fichier avant son écriture | Refuser un chemin absolu `/workspace/` dans un script destiné à l'hôte |
| **PostToolUse** | Le résultat, après coup | Lancer `shellcheck` sur un `.sh` qui vient d'être écrit |
| **Stop** | La réponse finale | Le `link-check` actuel : liens relatifs à la racine |

### Étape 3 — Prouver le hook avant de l'installer

Pour chaque candidat :

1. Écrire des cas de test avec les **exemples réels** de l'étape 1 : au moins
   un fautif et un correct.
2. **Rejouer** le hook sur l'historique des transcripts, puis compter :
   - combien de fautes il aurait attrapées ;
   - combien de **faux positifs** il aurait déclenchés.
3. Ne garder que les hooks qui attrapent au moins une vraie faute avec zéro
   faux positif. Un hook qui bloque à tort coûte plus cher que la faute qu'il
   évite.

### Étape 4 — Installer et suivre

- Un dossier `skills/<nom>.local/` par hook, avec son `hooks.json`, sur le
  modèle de `link-check`.
- Le message du hook dit **quoi corriger** et **comment**, dans la langue de
  l'utilisateur.
- Un hook doit rester rapide (moins de 100 ms), car il tourne à chaque appel
  d'outil.
- Une fois le hook en place, la leçon correspondante quitte le CLAUDE.md ou
  LESSONS. C'est du contexte en moins à chaque session.

### Ce qu'il ne faut pas transformer en hook

- **Les règles de jugement** : « demander l'intention avant de refaire un
  design », « un symptôme n'est pas un diagnostic ». Une expression régulière
  ne les vérifie pas. Elles restent du texte.
- **Les règles rares** : une erreur vue une seule fois ne justifie pas un hook
  qui tourne à chaque appel.
- **Les hooks Stop en cascade** : un hook Stop qui bloque oblige Claude à
  répondre de nouveau. Deux hooks qui se contredisent font tourner la session
  en boucle.

## 3. Premier tri, sur les règles de ce projet

Classement fait à la lecture des règles. Les occurrences restent à compter à
l'étape 1.

| Règle (source) | Verdict | Moment | Contrôle |
|---|---|---|---|
| Pas de `docker compose` sur tout le projet, jamais `down -v` (LESSONS + mémoire) | **Hook** | PreToolUse Bash | Bloquer `docker compose` sans nom de service, et `down -v` |
| Messages de commit sans numéro de session ni de phase (LESSONS + CLAUDE §10) | **Hook** | PreToolUse Bash | Sur `git commit`, chercher `session \d`, `phase \d`, `plans/` dans le message |
| Une commande de navigateur par appel Bash (LESSONS) | **Hook** | PreToolUse Bash | Bloquer `agent-browser … &&` |
| Jamais de chemin absolu en premier argument de `npx` (LESSONS) | **Hook** | PreToolUse Bash | `npx /` |
| Accolades autour de `$var` suivi d'un caractère non ASCII (LESSONS) | **Hook** | PostToolUse Write/Edit sur `.sh` | `\$[A-Za-z_]\w*[^\x00-\x7F]` |
| Pas de `#` dans les blocs shell des réponses (mémoire) | **Hook** | Stop | Ligne commençant par `#` dans un bloc ` ```bash ` ou ` ```zsh ` |
| Pas d'emoji, `⚠` compris (mémoire) | **Hook** | Stop | Plages Unicode des emoji et des pictogrammes |
| Ligne Recap de 80 caractères au plus (CLAUDE §14) | **Hook** | Stop | Présence et longueur de `**Recap** — …` |
| Liens relatifs à la racine (CLAUDE §15) | **Déjà fait** | Stop | `link-check` |
| Données de l'hôte anonymisées (mémoire) | **Partiel** | PreToolUse Write | Repérer `/Users/<nom>`, mais le nom réel reste à juger |
| Demander l'intention avant de refaire un design (mémoire) | **Leçon** | (n/a) | Jugement |
| Un symptôme persistant n'est pas un diagnostic (LESSONS) | **Leçon** | (n/a) | Jugement |

Huit règles sur douze peuvent devenir des hooks, et une neuvième l'est déjà.
Ce sont aussi celles que tu corriges le plus souvent à la main.

## Ordre conseillé

1. Écrire les hooks Bash du tableau. Ils sont les plus simples et les moins
   risqués : un bloc sur une commande précise.
2. Écrire les hooks Stop sur les réponses (emoji, `#` dans les blocs shell,
   Recap), en les rejouant d'abord sur les transcripts.
3. Ajouter à `/prepare-plan` la ligne **Repeated** et le tri des Gotchas.
4. Faire de la passe « corrections → hooks » un skill de l'image de base,
   une fois qu'elle a servi à la main sur ce projet.
