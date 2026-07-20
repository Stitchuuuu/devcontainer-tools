# Estimation heures-valeur

Analyse cette conversation et estime les "heures-valeur" pour la facturation freelance — c'est-a-dire le temps qu'un dev senior fullstack (Laravel/Vue, 8+ ans d'experience) mettrait pour accomplir ces taches SANS aucune assistance IA.

## Arguments

$ARGUMENTS

Parse les arguments (tous optionnels, separes par des espaces) :
- **Nombre <= 12** = heures par jour (defaut : lire config)
- **Nombre > 12** = TJM en EUR (defaut : lire config)

Exemples :
- `/user:hours` → tableau avec config par defaut
- `/user:hours 7` → base 7h/jour
- `/user:hours 475` → TJM 475 EUR
- `/user:hours 475 7` → TJM 475 + base 7h

## Etape 0 — Config et calibration

1. Lire la config depuis `/workspace/.devcontainer/skills/hours.local/hours.config.md`
   - Si absent, utiliser les defaults : TJM = 400 EUR, heures/jour = 6
   - Les arguments en ligne de commande overrident toujours la config

2. Verifier `/workspace/.devcontainer/skills/hours.local/hours-calibration.json`
   - Si le fichier date de plus de 30 jours ou n'existe pas, suggerer de lancer `/user:hours-calibrate`
   - Si disponible, utiliser la grille de calibration pour ancrer les estimations

## Etape 1 — Lister les taches

Analyser toute la conversation. Identifier chaque tache ou livrable distinct. Une "tache" est une unite de travail qu'un client reconnaitrait sur une feuille d'heures — pas des micro-etapes individuelles.

Grouper les prompts relies en taches coherentes. Par exemple :
- "Implementer le webhook Aeropay" (pas "lire le controller" + "ajouter la route" + "ecrire le test")
- "Rechercher et rediger l'analyse de tarification" (pas "chercher Malt" + "chercher CZSyn" + "compiler le tableau")

### Types de tache
Feature, Bug fix, Refactoring, Integration API, UI/CSS, Tests, Documentation, Recherche, Config/DevOps, Migration

## Etape 2 — Detecter les facteurs contextuels

Pour chaque tache, detecter les facteurs qui augmentent le temps necessaire :

| Facteur | Comment le detecter | Modificateur |
|---------|-------------------|-------------|
| **Stack legacy** | Vieux framework, pas de types, pas de tests, code spaghetti | +25 a +50% |
| **Criticite legale** | Paiement, RGPD, donnees sensibles, auth, conformite | +25 a +50% |
| **Code partage/couple** | DB partagee entre projets, effets de bord cross-repo | +15 a +30% |
| **Premiere fois** | Nouveau projet, onboarding implicite, pas de contexte | +20 a +40% |
| **API externe** | Dependance a une API tierce, docs variables, sandbox | +10 a +25% |

Plusieurs facteurs peuvent se cumuler.

## Etape 2b — Confirmation (texte libre)

Presenter les taches et facteurs detectes a l'utilisateur pour confirmation :

```
J'ai detecte les taches et facteurs suivants. Confirme ou ajuste :

| # | Tache                  | Type            | Facteurs detectes          |
|---|------------------------|-----------------|----------------------------|
| 1 | Webhook Aeropay        | Integration API | Paiement (+30%)            |
| 2 | Fix redirect           | Bug fix         | —                          |
| 3 | Migration DB           | Migration       | DB partagee (+25%)         |

Tu peux confirmer, ajouter/retirer des facteurs, ou corriger des taches.
```

Attendre la reponse avant de continuer.

## Etape 3 — Estimer les heures-valeur

Pour CHAQUE tache, estimer le temps qu'un dev senior mettrait SANS IA. Cela inclut le workflow professionnel complet :

1. **Comprehension** — Lire le ticket/spec, comprendre le contexte, identifier le code concerne
2. **Recherche** — Consulter APIs, docs, patterns, donnees marche si applicable
3. **Implementation** — Ecrire le code ou le contenu
4. **Review et refactoring** — Relecture, nettoyage, qualite
5. **Tests** — Tests manuels, edge cases, verifier que rien d'autre n'est casse
6. **Integration** — Commit, PR, documentation si applicable

NE PAS utiliser de multiplicateurs fixes. Estimer chaque tache independamment.

### Grille de calibration (fallback si pas de JSON)

```
CRUD endpoint + tests            2-4h
Integration API externe          4-8h
Bug fix simple                   1-2h
Bug fix complexe                 3-6h
Feature UI standard              2-4h
Feature UI complexe              4-8h
Refactoring composant            2-6h
Migration DB / upgrade           2-4h
Documentation technique          1-3h
Recherche / analyse              2-6h
Config CI/CD / DevOps            1-4h
```

### Garde-fous

- **Test du collegue** : un dev senior sans IA trouverait le ratio heures/travail raisonnable ?
- **Test du marche** : ces heures sont coherentes avec les estimations Malt/Codeur pour des taches similaires ?
- **Test de constance** : les heures mensuelles ne doivent pas exploser (120h avant → 200h apres = suspect)
- **Jamais depasser** l'estimation sans IA — on facture au prix du marche, pas au-dessus
- Heures arrondies au **0.5h** (minimum 0.5h par tache)

## Etape 4 — Tableau final

Toujours afficher ce tableau :

```
## Estimation heures-valeur — [date du jour]

| # | Tache              | Type      | Base | Contexte            | Heures | Jours |
|---|--------------------|-----------|------|---------------------|--------|-------|
| 1 | [Description]      | [Type]    | Xh   | [Facteur (+X%)]     | Xh     | X.XX  |
| 2 | [Description]      | [Type]    | Xh   | —                   | Xh     | X.XX  |
|   | **Total**          |           |      |                     | **Xh** | **X.XX** |
```

- **Jours** = heures / heures_par_jour (config ou argument), arrondi a 2 decimales
- **Base** = estimation avant application des facteurs contextuels
- **Heures** = base × (1 + somme des modificateurs)

Si TJM disponible (config ou argument), ajouter une ligne resume :
```
> Xh = X.XX jours (base Xh/jour) → **XXX EUR** au TJM de XXX EUR/jour
```

## Etape 5 — Temps reel (si logs disponibles)

Verifier si `/workspace/.devcontainer/skills/hours.local/logs/` contient un fichier log pour la session courante. Les logs sont stockes par session : `logs/<session_id>.jsonl`.

Pour trouver le bon fichier, lister les fichiers dans le dossier logs et identifier celui de la session courante (le plus recent, ou celui dont le session_id correspond au contexte).

Si un log existe, le lire et calculer :
- **Temps IA** : somme des intervalles `prompt` → `stop`
- **Temps humain** : somme des intervalles `stop` → `prompt` (gaps < 10 min)
- **Reflexion confirmee** : gaps >= 10 min marques comme reflexion (events `gap_answer`)
- **Pauses** : gaps >= 10 min marques comme pause (ou non resolus → demander)
- **Cout** : dernier event `stop` avec `cost_usd`

Afficher :

```
## Temps reel de session

### Timeline
HH:MM - HH:MM  [Phase description]          X min  (Tool1 xN, Tool2 xN)
HH:MM - HH:MM  Review humain                X min
HH:MM - HH:MM  ⏸️ [Reflexion/Pause]         X min

### Resume
| Mesure | Duree |
|--------|-------|
| Temps IA (agent actif) | X min |
| Temps humain (review, prompts) | X min |
| Reflexion confirmee | X min |
| Pauses exclues | X min |
| **Temps de travail effectif** | **X min** |

### Cout de session
| Mesure | Valeur |
|--------|--------|
| Tokens (cache read / output) | X.XM / XK |
| **Cout total** | **$X.XX** |

### Ratio de productivite
| Heures-valeur | Temps reel | **Ratio** |
|---------------|------------|-----------|
| Xh | X min | **X.Xx** |
```

La timeline est construite en clusterisant les tool uses par phase temporelle. Utiliser le contexte de la conversation pour nommer les phases intelligemment.

## Regles importantes

- Sortie en **francais** (descriptions, en-tetes de colonnes)
- Si la conversation ne contient que de la discussion/questions sans livrable reel, le dire honnetement au lieu de gonfler les heures
- Si l'estimation est incertaine, donner une fourchette (ex: "2-3h") et utiliser le point milieu pour les totaux
- Le temps de review/reponse lors de l'utilisation de `/user:hours` lui-meme est negligeable et n'est pas exclu
