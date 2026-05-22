# Calibration des donnees marche pour /user:hours

Met a jour la grille de reference des tarifs et estimations horaires en consultant les sources marche.

## Ce que fait cette commande

1. Consulter les sources marche ci-dessous via WebFetch
2. Extraire les TJM et grilles tarifaires a jour
3. Mettre a jour `/workspace/.devcontainer/skills/hours.local/hours-calibration.json`
4. Afficher un diff (ancien vs nouveau) des changements

## Sources a consulter

| Domaine | Donnee a extraire |
|---------|------------------|
| `www.malt.fr/t/barometre-tarifs/tech/developpeur-backend/developpeur-fullstack` | TJM moyen fullstack France |
| `www.codeur.com/developpeur/ia/tarif` | Tarifs dev par specialite |
| `czsyn.com/blog/tarif-developpeur-freelance-2026` | Grille TJM par techno et experience |
| `www.coqportage.fr/marche-freelance-it-et-conseil-2026/` | TJM par zone geo + impact IA |
| `www.rh-solutions.com/le-grand-guide-du-portage/tjm-freelances-it-pour-2026-les-vraies-tendances-du-marche/` | Tendances TJM IT annuelles |

## Format du fichier de sortie

Ecrire dans `/workspace/.devcontainer/skills/hours.local/hours-calibration.json` :

```json
{
  "last_updated": "YYYY-MM-DD",
  "sources": ["malt.fr", "codeur.com", "czsyn.com", "coqportage.fr", "rh-solutions.com"],
  "tjm_marche": {
    "fullstack_senior_province": { "min": 0, "max": 0, "median": 0 },
    "fullstack_senior_paris": { "min": 0, "max": 0, "median": 0 },
    "php_laravel_senior": { "min": 0, "max": 0, "median": 0 },
    "malt_fullstack_moyen": 0
  },
  "grille_heures": {
    "crud_endpoint": { "min": 2, "max": 4 },
    "integration_api": { "min": 4, "max": 8 },
    "bug_fix_simple": { "min": 1, "max": 2 },
    "bug_fix_complexe": { "min": 3, "max": 6 },
    "feature_ui_standard": { "min": 2, "max": 4 },
    "feature_ui_complexe": { "min": 4, "max": 8 },
    "refactoring": { "min": 2, "max": 6 },
    "migration": { "min": 2, "max": 4 },
    "documentation": { "min": 1, "max": 3 },
    "recherche": { "min": 2, "max": 6 },
    "devops": { "min": 1, "max": 4 }
  },
  "api_pricing": {
    "claude-opus-4-6": {"in": 15.00, "cache_read": 1.50, "cache_create": 18.75, "out": 75.00},
    "claude-sonnet-4-6": {"in": 3.00, "cache_read": 0.30, "cache_create": 3.75, "out": 15.00},
    "claude-haiku-4-5": {"in": 0.25, "cache_read": 0.03, "cache_create": 0.30, "out": 1.25}
  },
  "notes": ""
}
```

Remplir les valeurs `tjm_marche` avec les donnees extraites des sources. Pour la `grille_heures`, ajuster uniquement si les sources fournissent de nouvelles donnees (sinon conserver les valeurs par defaut). Pour `api_pricing`, verifier les tarifs actuels sur la page de pricing Anthropic (https://docs.anthropic.com/en/docs/about-claude/pricing). Les prix sont par MTok (million de tokens). Les cles du dict correspondent aux prefixes des noms de modeles dans l'API. Ajouter les nouveaux modeles si necessaire. Il n'existe pas d'API Anthropic pour les prix — il faut scraper la page ou les mettre a jour manuellement.

## Affichage du diff

Si un fichier existant est present, lire l'ancien d'abord et afficher les changements :

```
## Calibration mise a jour — [date]

### TJM marche
| Donnee | Ancien | Nouveau | Delta |
|--------|--------|---------|-------|
| Fullstack senior province (median) | 450 EUR | 465 EUR | +3.3% |
| ...

### Sources consultees
- malt.fr : OK (TJM moyen fullstack = 557 EUR)
- codeur.com : OK (grille par specialite)
- ...

Prochaine calibration recommandee : [date + 30 jours]
```

## Sortie en francais
