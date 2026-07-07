# Prompt — session de rectification préventive du outbound control channel

À coller **verbatim** dans une session Claude Code fraîche (nouveau
onglet, pas de contexte cumulé). Ne PAS l'invoquer via une commande
`/prepare-plan` ou similaire — c'est un audit ciblé, pas un rollout
multi-session. La session doit terminer en une passe, avec soit une
liste de rectifications proposées, soit un rapport « rien à corriger ».

---

## Prompt à coller

Tu es sur un devcontainer avec un patch récent du Claude Code VS Code
extension : un **canal de contrôle outbound** qui permet à un
contrôleur externe d'injecter des réponses à des permission requests
(comme si l'utilisateur avait cliqué Allow / Deny) via un fichier
JSONL. Le patch a été mergé en 3 pièces :

1. **`.devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py`**
   — instrumente `extension.js` : ajoute un file-watcher singleton dans
   `PanelManager.setupPanel(z,K,V,N){` (poll 200 ms sur
   `.devcontainer/logs/claude-code-vscode-ext-outbound.jsonl`), et
   instrumente `Comms.sendRequest(…)` pour logger les perm-requests
   pending dans `pending-perms.jsonl`.
2. **`.devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py`**
   — instrumente `webview/index.js` : intercepte un message
   `{type:"simulated_click", requestId, result}` dans le
   `window.addEventListener("message", …)`, cherche l'instance `Q =
   new Gn(requestId,…)` dans `this.permissionRequests.value` par
   `Q.channelId === requestId`, appelle `Q.accept(input, perms)` ou
   `Q.reject(msg, false)`.
3. **`.devcontainer/claude/outbound-tester.js`** — CLI node
   standalone : `list` (lit pending-perms.jsonl, affiche unsettled)
   et `send <rid> allow|deny [--input '<json>'] [--message '<txt>']`
   (append à outbound.jsonl).

Le tout est décrit en détail dans
[updates-v2.1/20260707-0811-vscode-ext-outbound-action-injector.md](updates-v2.1/20260707-0811-vscode-ext-outbound-action-injector.md).
Le plan de tests manuels est dans
[.devcontainer/claude/outbound-tester-manual-tests.md](.devcontainer/claude/outbound-tester-manual-tests.md).

**Ton mandat** : audit préventif ciblé sur les failure modes
plausibles, PAS une réécriture. Tu vas :

1. **Lire les 3 fichiers listés ci-dessus + les updates-v2.1 associés**
   pour reconstruire le modèle mental.

2. **Lire les injections finales** dans les fichiers patchés :
   ```bash
   grep -A 40 'notify-queue-outbound-inject-v1' \
     ~/.vscode-server/extensions/anthropic.claude-code-*/extension.js
   grep -A 30 'notify-queue-outbound-perm-log-v1' \
     ~/.vscode-server/extensions/anthropic.claude-code-*/extension.js
   grep -A 30 'notify-queue-outbound-perm-settle-v1' \
     ~/.vscode-server/extensions/anthropic.claude-code-*/extension.js
   grep -A 25 'notify-queue-webview-sim-click-v1' \
     ~/.vscode-server/extensions/anthropic.claude-code-*/webview/index.js
   ```
   Vérifier que le JS injecté est valide (`node --check`) et que la
   sémantique correspond aux intentions décrites.

3. **Chercher activement les failure modes suivants** :

   **F1 — Race au boot**. Le watcher est démarré dès la 1re invocation
   de `setupPanel()`, mais `this.sessionPanels.get(sid)` peut retourner
   `undefined` si outbound.jsonl a des lignes AVANT que la session
   cible ne soit ouverte. Vérifier : le watcher log un warning et
   continue ; le fichier n'est pas rewound pour retry.
   **Attendu** : ce cas est acceptable (log warning). Confirmer que
   c'est le comportement.

   **F2 — Bug de lookup Q.channelId**. La classe `Gn` stocke le
   requestId sous `.channelId` (nom trompeur — le param constructor
   est `$` capturé comme `this.channelId`). Vérifier que la comparaison
   webview `q?.channelId === rid` est correcte. Cross-check :
   `handleToolPermissionRequest($, Z, J) { … Q = new Gn($, …) }`
   → oui, `$` = requestId.

   **F3 — Q.accept / Q.reject args**. Confirmer que la signature
   coincide avec ce que le vrai click handler passe :
   - `Q.accept({}, [])` → equivalent d'un click Allow, `updatedInput`
     et `updatedPermissions` vides
   - `Q.reject("denied", false)` → equivalent d'un click Deny sans
     interrupt
   Compare avec le composant React qui gère les boutons (grep
   `permissionRequests.value` dans `webview/index.js`).

   **F4 — pending-perms.jsonl growth unbounded**. Rien ne wipe ce
   fichier en cours de session (uniquement au boot par
   `post-start.sh`). Pour un usage prolongé (session de plusieurs
   heures), calcule combien de lignes ça fait par heure. Si > 1000/h,
   propose un rotate ou un truncate périodique. Sinon RAS.

   **F5 — Idempotence broken sur re-run**. Confirmer que
   `run-all.sh` re-appliqué N fois n'ajoute jamais un 2ème watcher
   (le flag `this._outboundStarted` protège). Réel : lancer 3× et
   vérifier `grep -c 'setInterval' extension.js` avant/après (ne
   doit pas augmenter à chaque run).

   **F6 — Concurrent writes à outbound.jsonl**. Deux invocations
   simultanées de `outbound-tester.js send` peuvent-elles interleave ?
   `fs.appendFileSync` sur POSIX est atomique jusqu'à `PIPE_BUF` (4 KB
   généralement). Confirmer que chaque ligne JSON reste sous ça et
   qu'on write avec `\n` en fin.

   **F7 — Le watcher lit des lignes partielles**. Si un writer append
   la moitié d'une ligne au moment où le watcher poll, le `\n` n'est
   pas là — le `JSON.parse` fail. Vérifier le buffering : le patch
   utilise un `carry` string qui garde le trailing partial jusqu'au
   prochain poll. Confirmer qu'il est correctement séparé (le
   dernier élément de `split("\n")` doit aller dans `carry`).

   **F8 — Le webview intercept bloque des messages légitimes**. Le
   check `if(_m?.type === "simulated_click")` doit être exclusif : si
   la condition matche un message légitime avec ce type par
   coïncidence, on shortcut avant l'enqueue. Confirmer que
   `"simulated_click"` n'est pas un type utilisé par Anthropic ailleurs
   (grep dans webview/index.js pour le confirmer).

   **F9 — sendRequest instrumentation loggue TOUT** dans pending-perms.
   Le patch conditionne sur `K?.type === "tool_permission_request"`
   mais un typo dans la comparaison ferait tout logger. Confirmer.

   **F10 — Multi-session cross-talk**. Un send vers sessionId=A avec
   requestId=B, où B appartient à la session A → OK. Mais si B est
   dans session C (mismatched pair), notre lookup
   `this.sessionPanels?.get(sid_A)` renvoie panel A, on postMessage à
   panel A qui cherche B dans SES permissionRequests, ne le trouve
   pas, warn console. Confirmer que ce cas est bien un warn et pas
   un crash / cross-inject.

   **F11 — Le webview intercept ne handle pas les erreurs**. Si
   `Q.accept` throw (par exemple `updatedInput` invalide), le catch
   est-il en place ? Grep pour `catch` dans l'injection webview.

   **F12 — Le tester CLI ne handle pas les paths absolus**. Si l'user
   lance `node /path/to/outbound-tester.js` depuis un CWD différent,
   les paths des JSONL sont-ils résolus correctement ? Le patch
   utilise `path.resolve(__dirname, '..', 'logs')` — vérifier que
   c'est stable.

4. **Rapporte** :

   - Pour chaque F qui est un vrai bug : file:line + fix proposé (pas
     de code encore, juste le diff conceptuel).
   - Pour chaque F qui est un edge case acceptable : justifier
     brièvement.
   - Si tu identifies un failure mode HORS liste : ajoute-le comme
     F13, F14, …
   - Si tout est clean : dis-le explicitement, ne bricole pas des
     problèmes.

5. **Ne modifie AUCUN fichier** avant qu'on ait discuté du rapport.
   L'output attendu est un audit .md brut, pas un commit.

**Contraintes** :
- Utilise les subagents `Explore` pour les greps qui prennent du
  temps ; garde le main thread pour la synthèse.
- Pas de web fetch. Tout est on-disk.
- Reste focalisé sur les 3 fichiers du canal outbound + leur
  environnement immédiat (post-start.sh, la config JSONL). Le reste
  du repo n'est pas dans le scope.

---

## Mes remarques (à compléter avant de coller)

<!-- Ajoute ici les points que tu veux que la session cible en priorité,
     par exemple :
     - "surtout regarde le F4 en profondeur, on va laisser tourner 8h+"
     - "j'ai déjà vu un warning [outbound-inject] no panel for sid=... hier,
        peux-tu tracer d'où ça peut venir ?"
     - "on va aussi utiliser ça sur des ExitPlanMode / AskUserQuestion, est-ce
        que la sémantique tient ?"
-->

