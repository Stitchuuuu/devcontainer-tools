# 20260707-0739 — Disable auto-redirect to login page in VS Code Claude Code extension

**Affects** : v2.1 devcontainers where the Claude Code VS Code extension
webview auto-redirects to the login page after `/logout`, OAuth token
expiry, or any unauthenticated state.

**Symptom** : during a session, hitting `/logout` (or letting the token
expire) instantly swaps the chat webview for the "Sign in to Claude"
page. No way to dismiss it, no way to keep browsing the last chat.

**Cause** : the extension enables login prompting by default. The
extension bundle exposes an official setting `claudeCode.disableLoginPrompt`
(documented in the extension's `package.json`) that flips this behavior
off — when true, the extension treats authentication as "handled
externally" and stops pushing the login view. **Webview only** — the
CLI `claude` in terminal is a separate binary and ignores VS Code
settings entirely.

**Trade-off** : no in-extension login button either (there is no
`claude-vscode.login` command in the bundle, only `.logout`). Re-login
is done via CLI : run `claude` in a terminal → OAuth flow → tokens land
in `~/.claude/` → reload VS Code window (`Developer: Reload Window`)
so the extension picks up the fresh cache.

## Manual how-to

Two files, same edit. This is pure JSONC — no daemon, no rebuild, but
you MUST rebuild the devcontainer afterwards for VS Code to reapply
the machine settings.

### File 1 — `.devcontainer/devcontainer.json`

Open the file in your editor. Locate the block :

```
      "settings": {
        ...
        "window.title": "Devcontainer Tools [${localWorkspaceFolderBasename}] - Claude Code Sandbox - ${activeEditorShort}",
        "claudeCode.preferredLocation": "primary"
      }
```

- Add a comma at the end of the `"claudeCode.preferredLocation": "primary"` line.
- On the next line (same indentation, 8 spaces), insert :

```
        "claudeCode.disableLoginPrompt": true
```

Final shape :

```
      "settings": {
        ...
        "claudeCode.preferredLocation": "primary",
        "claudeCode.disableLoginPrompt": true
      }
```

### File 2 — `templates/v2/devcontainer.json`

**Same edit, same location.** The template mirrors the dogfood
devcontainer so downstream projects generated from `templates/v2/`
inherit the setting.

Open `templates/v2/devcontainer.json` → locate the block :

```
      "settings": {
        ...
        "window.title": "{{PROJECT_DISPLAY_NAME}} [${localWorkspaceFolderBasename}] - Claude Code Sandbox - ${activeEditorShort}",
        "claudeCode.preferredLocation": "primary"
      }
```

Apply the same two changes : add the comma, insert the new line.

### Commit

`````bash
git add .devcontainer/devcontainer.json templates/v2/devcontainer.json

git commit -m "feat(vscode-ext): disable auto-redirect to login page in webview"
`````

### Rebuild the container

VS Code palette → `Dev Containers: Rebuild Container`. On next boot, the
`customizations.vscode.settings` block gets flattened into
`~/.vscode-server/data/Machine/settings.json`.

## Verify

- [ ] `grep disableLoginPrompt .devcontainer/devcontainer.json`
      → 1 hit with value `true`.
- [ ] `grep disableLoginPrompt templates/v2/devcontainer.json`
      → 1 hit with value `true`.
- [ ] After rebuild : `grep disableLoginPrompt ~/.vscode-server/data/Machine/settings.json`
      → 1 hit with value `true`.
- [ ] In the Claude Code webview, run `/logout`. Expected : the webview
      does NOT redirect to the login page. A neutral "authentication is
      handled externally" state is shown (no login button).
- [ ] Non-regression : in a terminal, run `claude` → CLI login flow
      prompts normally, unaffected by the setting.
- [ ] Re-login path : `claude` in terminal → OAuth flow → reload window
      (palette → `Developer: Reload Window`) → webview reconnects
      with the new tokens.

## Rollback

Reverse the two edits (drop the `"claudeCode.disableLoginPrompt": true`
line, remove the trailing comma on `"claudeCode.preferredLocation"`),
then rebuild the container.

`````bash
git revert <commit-hash>
`````
