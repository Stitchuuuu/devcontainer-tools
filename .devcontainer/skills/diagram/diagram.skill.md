---
description: |
  Generate an .excalidraw file (Excalidraw v2 JSON) from an NL description, an
  ASCII sketch, or a node/edge structure. Writes the file at the path requested
  by the user via the Write tool. Renders it to SVG + PNG headlessly on request,
  via the bundled scripts/export.mjs — no Excalidraw app needed.

  Auto-trigger : "fais-moi un diagramme excalidraw", "génère un .excalidraw",
  "draw this as excalidraw", "schéma excalidraw de X", "convertis ce Mermaid
  en .excalidraw", "diagramme d'architecture excalidraw", "turn this ASCII
  into excalidraw", "exporte ce diagramme en png", "rends-moi le svg",
  "export the diagram", "génère l'image du diagramme", "convertis ce
  .excalidraw en png".
argument-hint: "<description | ASCII | Mermaid> → <path/out>.excalidraw"
---

# /diagram — `.excalidraw` generation, and headless SVG/PNG export

**The two halves have different costs — keep them distinct.**

**Generation is dependency-free.** An `.excalidraw` file is just a v2 JSON whose
schema has been stable since 2021. This skill bundles everything needed for an
LLM to write a valid file in one shot with the Write tool — no Node tooling, no
install, no Dockerfile or firewall changes.

**Export is not.** [`scripts/export.mjs`](./scripts/export.mjs) renders
`.excalidraw` → SVG + PNG @2x headlessly, dark by default, with the handwritten
fonts embedded. It needs four npm packages, installed **inside this skill's own
directory** rather than in the project manifest, plus a one-off host-side font
install for the PNG. Both are covered in
[Export SVG/PNG](#export-svgpng) — read it before running the script, and before
telling a user their diagram "can't be rendered here".

Export is **on demand**, never automatic : writing a diagram produces one file,
not three. Offer it in the recap and wait.

## ⚠️ Mandatory step 0 — re-read `KNOWLEDGE.md`

**Before any design work, read
[`KNOWLEDGE.md`](./KNOWLEDGE.md) in this skill's directory.** That file
accumulates rules distilled from past mistakes (arrow label spacing, font
consistency, dashed frames, color palette, z-order). Skipping it = falling
back into the same traps. The conventions below remain the schema reference ;
KNOWLEDGE.md adds the **readability** and **workflow** rules that the schema
alone doesn't enforce.

---

## When to use

- Architecture diagram (≤ 15 nodes), simple flow, short sequence.
- ASCII / simple Mermaid → editable `.excalidraw` conversion.
- The user wants a re-editable file rather than a frozen image.
- The user wants an SVG / PNG → generate, then run
  [`export.mjs`](#export-svgpng). Also the entry point when the `.excalidraw`
  already exists and only the image is missing.

## When NOT to use

- The graph exceeds ~20 nodes → suggest Mermaid + desktop app import
  (`File → Import → from Mermaid`). Manual layout becomes painful.
- Non-flowchart diagram (sequence, class, ER) — `.excalidraw` is freeform,
  but auto-layout for these types is out of scope.

---

## Excalidraw v2 schema — envelope

```json
{
  "type": "excalidraw",
  "version": 2,
  "source": "claude-diagram-skill",
  "elements": [ /* … */ ],
  "appState": {
    "viewBackgroundColor": "#ffffff",
    "gridSize": 20,
    "gridStep": 5,
    "gridModeEnabled": false,
    "lockedMultiSelections": {}
  },
  "files": {}
}
```

`type` / `version` are required. `source` is free-form. `files` is `{}`
unless you embed images (out of scope).

Those five `appState` keys are **exactly** what the app writes back on
export — emit them all, or the file diverges from itself at the first save.
`gridSize: 20` + `gridModeEnabled: false` isn't cosmetic : the layout
convention below is already "multiples of 20", so the app's grid lines up
with the authoring grid the day someone toggles it on.

**No `theme` key** — and never add one. See
[KNOWLEDGE L14](./KNOWLEDGE.md) : the theme belongs to the reader, not the
document.

---

## Fields common to every element

Every element must carry these fields (otherwise the app rejects or
regenerates them) :

```jsonc
{
  "id": "el-001",                  // unique string. "el-NNN" sequential is fine.
  "type": "...",                   // rectangle | ellipse | diamond | arrow | line | text
  "x": 0, "y": 0,                  // top-left corner (text: baseline-box position)
  "width": 0, "height": 0,
  "angle": 0,                      // radians ; always 0 unless explicit rotation
  "strokeColor": "#1e1e1e",
  "backgroundColor": "transparent",
  "fillStyle": "solid",            // solid | hachure | cross-hatch
  "strokeWidth": 2,                // 1 (thin) | 2 (medium) | 4 (thick)
  "strokeStyle": "solid",          // solid | dashed | dotted
  "roughness": 1,                  // 0 (none) | 1 (default) | 2 (cartoonist)
  "opacity": 100,                  // 0-100
  "groupIds": [],
  "frameId": null,
  "roundness": null,               // {"type": 3} on rectangle for rounded corners
  "seed": 100001,                  // random int, may be sequential
  "version": 1,                    // always 1 at creation
  "versionNonce": 100001,          // random int
  "isDeleted": false,
  "boundElements": [],             // [{type:"text"|"arrow", id}] — see Bindings
  "updated": 1717689600000,        // ms timestamp (exact value doesn't matter)
  "link": null,
  "locked": false,
  "index": "a0"                    // fractional index (z-order). a0,a1,...,a9,aA,aB,...
}
```

`index` : use a lex-sortable string. Simple convention : `a0`, `a1`, …, `a9`,
`aA`, …, `aZ`, `b0`, … Roughly 50 elements fit into `a0`-`a9` + `aA`-`aZ` +
`b0`-`bN`. The app regenerates if invalid but it's cleaner to provide valid
indices up front. For background elements (frames, backdrops) use `Z*`
prefixed indices (e.g., `Zy`, `Zz`) — ASCII `Z` sorts before `a`, so they
render behind everything else (see [KNOWLEDGE L05](./KNOWLEDGE.md)).

---

## Type-specific fields

### Rectangle / Ellipse / Diamond

Common fields above plus :

```jsonc
{
  "type": "rectangle",  // or "ellipse" / "diamond"
  "roundness": {"type": 3}  // rect only, for rounded corners. null otherwise.
}
```

### Text

```jsonc
{
  "type": "text",
  "text": "hook.js",
  "fontSize": 20,                  // 16 | 20 | 28 | 36
  "fontFamily": 5,                 // 5=Excalifont (handwritten) | 7=Cascadia (mono) | 8=Lilita
  "textAlign": "center",           // left | center | right
  "verticalAlign": "middle",       // top | middle | bottom
  "baseline": 18,                  // ≈ fontSize - 2. App recomputes anyway.
  "lineHeight": 1.25,
  "containerId": "rect-id-or-null", // ID of the rect/ellipse/diamond holding this text
  "originalText": "hook.js",       // always = text at creation
  "autoResize": true               // mandatory, always true. Keep it LAST.
}
```

`autoResize` is missing from no-longer-current examples. Absent, the app
injects it on open — so the file rewrites itself at the first save and the
diff is pure noise. Emit it on **every** text element, last key, which is
where the app puts it.

For font choice, follow [KNOWLEDGE L03](./KNOWLEDGE.md) : **mono (7) for any
code identifier**, **handwritten (5) only for prose and zone headers**.

For the `text` **content**, stay inside ASCII + Latin-1 — see the
[glyph rule](#glyph-coverage--hard-rule). Anything outside renders as a tofu
box in the PNG, and only in the PNG.

### Arrow / Line

```jsonc
{
  "type": "arrow",
  "points": [[0, 0], [dx, dy]],    // min 2 points : [0,0] then delta to end
  "lastCommittedPoint": null,
  "startBinding": {                // null for floating arrow
    "elementId": "rect-from-id",
    "focus": 0,                    // -1 to 1, relative position on the edge. 0 = middle.
    "gap": 8                       // padding between rect edge and arrow start
  },
  "endBinding": { "elementId": "rect-to-id", "focus": 0, "gap": 8 },
  "startArrowhead": null,          // arrow | bar | dot | triangle | null
  "endArrowhead": "arrow",
  "elbowed": false,                // true = 90° polyline, false = straight
  "roundness": {"type": 2}         // arc smoothing. {"type": 2} = rounded.
}
```

For an arrow's `width` / `height` : absolute delta values (never negative).
`x, y` = position of the `[0,0]` point in `points`.

> **Do not "modernise" the bindings.** On open, the app migrates
> `{elementId, focus, gap}` to `{"mode": "orbit", "elementId": …,
> "fixedPoint": [x, y]}` — you will see that form in any file that has been
> through excalidraw.com. **Keep emitting `focus`/`gap` anyway.** The
> `fixedPoint` values are solved from the geometry, not readable
> coordinates : an observed migration produced `[0.5001, 0.5001]` for one
> centre binding and `[0.3996…, 0.6003…]` / `[0.2419…, 0.2419…]` for
> another. Hand-written, they pin the arrow to an arbitrary spot.
> `focus: 0, gap: 8` means "centre to centre, 8px clearance" and survives
> the migration untouched. See [KNOWLEDGE L15](./KNOWLEDGE.md).

---

## Bindings — critical rules

### Text inside a shape (text ↔ shape)

```jsonc
// Rect declares it contains the text
{
  "id": "rect-hook",
  "type": "rectangle",
  "boundElements": [
    {"type": "text", "id": "text-hook"}
  ]
}

// Text points back to its container
{
  "id": "text-hook",
  "type": "text",
  "containerId": "rect-hook",
  "textAlign": "center",
  "verticalAlign": "middle"
}
```

Without this reciprocal binding the text isn't attached to the box —
moving the box leaves the text behind → broken diagram on edit.

### Arrow between two shapes (arrow ↔ shape)

```jsonc
// Each endpoint rect must reference the arrow
{
  "id": "rect-hook",
  "boundElements": [
    {"type": "text", "id": "text-hook"},
    {"type": "arrow", "id": "arrow-hook-queue"}
  ]
}

{
  "id": "rect-queue",
  "boundElements": [
    {"type": "text", "id": "text-queue"},
    {"type": "arrow", "id": "arrow-hook-queue"}
  ]
}

// Arrow references both rects
{
  "id": "arrow-hook-queue",
  "type": "arrow",
  "startBinding": {"elementId": "rect-hook",  "focus": 0, "gap": 8},
  "endBinding":   {"elementId": "rect-queue", "focus": 0, "gap": 8}
}
```

Without binding, moving a node leaves the arrow in place → broken. And
`focus`/`gap` is the form to write — never the migrated `fixedPoint` one,
see the note under [Arrow / Line](#arrow--line).

### Arrow label (text ↔ arrow)

```jsonc
// Arrow holds its label
{
  "id": "arrow-hook-queue",
  "boundElements": [{"type": "text", "id": "text-arrow-label"}]
}

// Label points to the arrow as container
{
  "id": "text-arrow-label",
  "type": "text",
  "text": "fs.watch",
  "containerId": "arrow-hook-queue",
  "textAlign": "center",
  "verticalAlign": "middle"
}
```

The label auto-positions at the arrow's midpoint.

---

## Layout conventions (must respect)

| Element | Standard size |
|---|---|
| Rectangle (node) | `w=240, h=80` (or `w=120, h=60` for dense channel boxes) |
| Diamond (decision) | `w=160, h=100` |
| Ellipse (state, IO) | `w=160, h=80` |
| Spacing between nodes | `min 80px` horizontal, `120px` vertical when arrow has a label (see [KNOWLEDGE L01](./KNOWLEDGE.md)) |
| Grid | all coords multiples of `20` |

For vertical separation between event rows and consumer/channel rows, leave
**at least 200px** of gap (see [KNOWLEDGE L02](./KNOWLEDGE.md)).

Colors (Excalidraw standard palette, see also [KNOWLEDGE L07](./KNOWLEDGE.md)) :
- Stroke : `#1e1e1e` (always for nodes), `#868e96` (gray) for dashed zone frames
- Categorical backgrounds :
  - `#a5d8ff` (blue) = event sources / IO
  - `#ffec99` (yellow) = files / data
  - `#b2f2bb` (green) = process / module
  - `#ffc9c9` (pink) = central bus / message hub
  - `transparent` = consumer / output
- Always `fillStyle: "solid"` for categorical backgrounds (readability).

Arrows : `endArrowhead: "arrow"`, `startArrowhead: null`,
`roundness: {"type": 2}`.

---

## Copy-paste templates

### T1 — Box with label

```jsonc
[
  {
    "id": "rect-1", "type": "rectangle",
    "x": 100, "y": 100, "width": 240, "height": 80,
    "angle": 0, "strokeColor": "#1e1e1e", "backgroundColor": "#a5d8ff",
    "fillStyle": "solid", "strokeWidth": 2, "strokeStyle": "solid",
    "roughness": 1, "opacity": 100, "groupIds": [], "frameId": null,
    "roundness": {"type": 3}, "seed": 100001, "version": 1, "versionNonce": 100001,
    "isDeleted": false, "boundElements": [{"type": "text", "id": "text-1"}],
    "updated": 1717689600000, "link": null, "locked": false, "index": "a0"
  },
  {
    "id": "text-1", "type": "text",
    "x": 130, "y": 130, "width": 180, "height": 20,
    "angle": 0, "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
    "fillStyle": "solid", "strokeWidth": 2, "strokeStyle": "solid",
    "roughness": 1, "opacity": 100, "groupIds": [], "frameId": null,
    "roundness": null, "seed": 100002, "version": 1, "versionNonce": 100002,
    "isDeleted": false, "boundElements": [], "updated": 1717689600000,
    "link": null, "locked": false, "index": "a1",
    "text": "hook.js", "fontSize": 20, "fontFamily": 7,
    "textAlign": "center", "verticalAlign": "middle", "baseline": 18,
    "lineHeight": 1.25, "containerId": "rect-1", "originalText": "hook.js",
    "autoResize": true
  }
]
```

### T2 — Two boxes connected by an arrow

Copy T1 twice (ids `rect-1`/`text-1` and `rect-2`/`text-2`, different
x coords), then add :

```jsonc
{
  "id": "arrow-1-2", "type": "arrow",
  "x": 340, "y": 140,                        // [0,0] corner of points
  "width": 100, "height": 0,                 // absolute delta to end
  "angle": 0, "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
  "fillStyle": "solid", "strokeWidth": 2, "strokeStyle": "solid",
  "roughness": 1, "opacity": 100, "groupIds": [], "frameId": null,
  "roundness": {"type": 2}, "seed": 100003, "version": 1, "versionNonce": 100003,
  "isDeleted": false, "boundElements": [], "updated": 1717689600000,
  "link": null, "locked": false, "index": "a4",
  "points": [[0, 0], [100, 0]],
  "lastCommittedPoint": null,
  "startBinding": {"elementId": "rect-1", "focus": 0, "gap": 8},
  "endBinding":   {"elementId": "rect-2", "focus": 0, "gap": 8},
  "startArrowhead": null, "endArrowhead": "arrow", "elbowed": false
}
```

**Don't forget** to add `{"type": "arrow", "id": "arrow-1-2"}` to
`boundElements` of `rect-1` AND `rect-2`.

### T3 — Decision (diamond) → 2 branches

Diamond instead of rectangle (`type: "diamond"`, `roundness: null`, size
`160×100`). Two outgoing arrows to 2 rects, each labelled (yes/no) via a
text with `containerId` = arrow id.

### T4 — Cluster (4-5 nodes in a grid)

Horizontal or vertical grid. Keep at least 60px between edges. All arrows
in `elbowed: false` mode to stay readable.

### T5 — Dashed zone frame

```jsonc
{
  "id": "frame-zone", "type": "rectangle",
  "x": 70, "y": 20, "width": 300, "height": 740,
  "angle": 0, "strokeColor": "#868e96", "backgroundColor": "transparent",
  "fillStyle": "solid", "strokeWidth": 1, "strokeStyle": "dashed",
  "roughness": 1, "opacity": 100, "groupIds": [], "frameId": null,
  "roundness": {"type": 3}, "seed": 100099, "version": 1, "versionNonce": 100099,
  "isDeleted": false, "boundElements": [], "updated": 1717689600000,
  "link": null, "locked": false, "index": "Zy"
}
```

`strokeStyle: "dashed"`, gray stroke, no fill, behind everything (`Zy`/`Zz`).
Use one per logical zone (e.g., client vs server, app vs database).

---

## Bundled scripts — for series and regeneration

[`scripts/`](./scripts/) ships four **generic, project-agnostic** executable
Node scripts. The first three are zero-dependency ; `export.mjs` is the one
that needs an install (see [Export SVG/PNG](#export-svgpng)). Hand-written JSON
via Write stays the default for a single small diagram ; reach for the scripts
when the job is a **series** (several related diagrams), a **regeneration**
(specs likely to be edited and re-run), or anything near the ~20-node ceiling —
one spec file then beats hand-maintaining thousands of JSON lines.

| Script | Role |
|---|---|
| `exca.mjs` | Builder library (`import { D }`) : auto-sized nodes on the 20px grid, reciprocal bindings (text↔shape, arrow↔shape, label↔arrow), dashed zones with `Z*` z-order, `AIDE`-style help boxes. Encodes L01/L05/L13/L15 by construction. |
| `check.mjs` | `node check.mjs <dir \| files...>` — re-reads files **from disk** (a builder bug must not mask an output bug) : envelope, reciprocal bindings, grid, L01/L12/L13, 2D overlaps, zone straddling, z-order. `--neg` injects 3 defects and expects ≥ 4 findings — run it once per session ; a checker without a negative control proves nothing. |
| `merge.mjs` | `node merge.mjs <out> <in...> [--gutter=N]` — merges several `.excalidraw` into horizontal bands : ids prefixed, indices regenerated, arrow bboxes computed from their `points` (an arrow's `x` is its start point, not its left edge). |
| `export.mjs` | **The only one with dependencies.** `.excalidraw` → `.svg` + `@2x.png`, dark by default, fonts embedded. Full section below. |

Spec files live **next to their output** (e.g.
`docs/…/variantes-100/generate.mjs`), import the builder from here, and
document their own regen commands in their header. The scripts do not
replace the visual check in the app (L08) — they only make everything
*before* it mechanical.

---

## Export SVG/PNG

```sh
node .devcontainer/skills/diagram/scripts/export.mjs <file.excalidraw | dir> [options]
```

Writes `<name>.svg` and `<name>@<scale>x.png` **next to the source**, or into
`--out`. A directory argument expands to its `*.excalidraw` children (not
recursive). Runs from any CWD.

| Flag | Default | Effect |
|---|---|---|
| `--out <dir>` | next to source | write elsewhere (created if missing) |
| `--scale <n>` | `2` | PNG factor. Carried by the `<svg>` width/height, viewBox kept |
| `--bg <color>` | none | opaque background, composited **after** the dark filter |
| `--transparent` | off | no background at all |
| `--svg-only` | off | skip rasterization — the only mode that runs without `sharp` |
| `--light` | off | disable dark rendering. **Dark is the default** |

Dark is a render filter, not a colour set — `export.mjs` reimplements
`invert(93%) hue-rotate(180deg)` on the raw pixels. Never touch the sources'
`viewBackgroundColor` to "make a dark version" ; see
[KNOWLEDGE L14](./KNOWLEDGE.md).

### Font prerequisite — PNG only

The PNG is rasterized by librsvg, which reads fonts through fontconfig, which
only indexes TTF/OTF. Excalidraw ships woff2 only.
[`scripts/install-excalidraw-fonts.sh`](../../../scripts/install-excalidraw-fonts.sh)
closes that gap : it converts to TTF via `wawoff2` and writes `/etc/fonts/fonts.conf`,
which this image doesn't have. It runs **from the host** (`docker exec -u root`) —
there is no `apt-get` in the container, the firewall allows no Debian repo, and
`sudo` asks for a password.

**Recognise the symptom : SVG perfect, PNG full of tofu boxes = fonts missing.**
The SVG embeds its own `@font-face` rules, so it never shows the defect.

```sh
ls /usr/local/share/fonts/excalidraw   # non-empty → fonts installed
```

Check this *before* concluding the exporter is broken.

### Glyph coverage — hard rule

The installed fonts cover 1728 codepoints, but Excalifont itself is poor outside
ASCII + Latin-1. **Keep generated label text to ASCII + Latin-1.**

| Glyphs | PNG result |
|---|---|
| `ʳ` `ᵉ` `⟨` `⟩` `∝` | **tofu** — no installed font contains them. Never emit these |
| `≥` `→` `≈` | OK via fontconfig fallback to Liberation Sans — different weight, acceptable |
| `«` `»` `‹` `›` `~` `(` `)` `<` `>` `=` | OK in Excalifont **and** Lilita One |

A browser always saves the SVG with a system fallback, so **the defect only ever
appears in the PNG**. Don't rely on looking at the SVG to catch it.

### Dependencies

`export.mjs` needs `@excalidraw/excalidraw`, `react`, `react-dom` and `jsdom`.
React is dead weight — loaded, never rendered — but unavoidable : the Excalidraw
bundle has a single `exports` entry and ESM evaluation is all-or-nothing.

They live in **this skill's own** [`package.json`](./package.json) and
`node_modules/`, deliberately **not** in the project manifest :

- **Adding `react` to the root devDependencies auto-enables Biome 2's React
  domain**, which then applies `lint/correctness/useHookAtTopLevel` to Vue
  composables — 29 lint errors across 19 untouched `services/doctor/**` files.
  Neutralising it needs `linter.domains.react: "none"` in `biome.json`, i.e.
  degrading the whole project's lint config for one export script. Settled
  decision — do not "simplify" it back.
- **`sharp` is absent on purpose** and resolves by walking up to the repo root,
  where it is already a *production* dependency. It ships per-platform native
  binaries, and the root `postinstall`
  (`scripts/install-cross-arch-natives.mjs`) repairs them for both sides of the
  macOS↔Linux bind mount. A skill-local copy would carry only the installing
  side's binary, with no postinstall to fix it.
- **Versions are pinned exact and the lockfile is committed.** Nothing watches
  this directory : no root `npm ci`, no Biome (`files.includes` is an allowlist
  and `.devcontainer/**` is not in it), no dependency extract (see below). The
  lockfile is the only reproducibility anchor. This is a deliberate exception to
  the "latest stable" rule in
  [.claude/rules/dependencies.md](../../../.claude/rules/dependencies.md) — don't
  "helpfully" bump them.

Reinstall :

```sh
npm install --prefix .devcontainer/skills/diagram
```

**Use `--prefix`, don't `cd`.** npm resolves its `localPrefix` by walking up to
the first directory holding a `package.json` *or* a `node_modules` — from inside
the skill before its manifest exists, that resolves to the repo root and installs
there, which is exactly the forbidden outcome.

Two known frictions with that command :

- **The firewall's npm allowlist can never learn these packages.** The extractor
  (`skills/scan-deps/extractors/lib/common.sh`) loops `for depth in 3 5 8 10` and
  returns on the first non-empty result — the root manifest matches at depth 1,
  so this one at depth 4 is never reached. Installing works in `basic` mode
  (the current default) ; in strict mode every tarball 403s.
- **`EACCES` on `/home/node/.npm/_cacache`** means root-owned cache entries, left
  by an older `install-excalidraw-fonts.sh` that ran `npm i -g wawoff2` as root
  without `--cache`. That script now uses its own cache dir. To get past an
  already-polluted cache, add `--cache <a writable dir>` to the install.

### Known-harmless noise

- `Failed to fetch font family … esm.sh … fetch failed` — Excalidraw's native
  font inlining trying the CDN from behind the firewall. The script reinjects the
  9 `@font-face` rules itself from the local bundle, so the SVG is complete.
  `export.mjs` now silences this exact message. **Do not open the firewall for
  it** — that would trade a deterministic offline export for a network
  dependency.
- `npm audit` reports 9 transitive advisories (`lodash-es`, `nanoid`, the
  `chevrotain`/`langium` chain), all reached through
  `@excalidraw/mermaid-to-excalidraw`. The export path calls only `exportToSvg`
  and never the Mermaid parser. npm's sole offered "fix" is a semver-major
  **downgrade** to `@excalidraw/excalidraw@0.17.6`, which is not one.

## Output workflow

When the user requests a diagram :

1. **Read [`KNOWLEDGE.md`](./KNOWLEDGE.md)** (mandatory, every time).
2. **Ask for the output path** if not provided. No default — the user
   decides where to store (`docs/img/`, `tmp/`, etc.).
3. **Verify the parent dir exists** via `ls`. If not, ask the user to
   create it or change the path — no silent `mkdir -p`.
4. **Design the element array** respecting bindings (text↔shape,
   arrow↔shape, arrow↔label) AND the grid layout (multiples of 20). Run
   through KNOWLEDGE.md rules one by one before writing.
5. **Write the file** via the Write tool, JSON indented at 2 spaces.
6. **Quick validate** — the last number must be `0` (every text carries
   `autoResize`) :
   ```sh
   jq '.type, .version, (.elements | length),
       ([.elements[] | select(.type == "text" and (has("autoResize") | not))] | length)' <file>
   ```
7. **Recap to user** with explicit ask to open in app and verify visually,
   and an **offer** to export — never an export already done (it would write
   two more files next to the source, unasked) :
   ```
   Wrote <path> — N elements (X rects, Y arrows, Z texts).
   Open in excalidraw.com (drag & drop) or the desktop app.
   Test : move a node — arrows + labels should follow.
   Want the SVG + PNG @2x ? Say so and I'll run the exporter.
   ```
8. **Export only if asked** — then run
   [`export.mjs`](#export-svgpng) and report the dimensions it prints.

---

## Limits to surface to the user

- **The PNG needs a host-side font install**, done once per container. Without
  it the SVG is still perfect and the PNG is full of tofu — see
  [Font prerequisite](#font-prerequisite--png-only).
- **Label text is limited to ASCII + Latin-1** for the PNG to render — see
  [Glyph coverage](#glyph-coverage--hard-rule). Say so if the user asks for a
  glyph on the forbidden list rather than emitting a silent tofu.
- **The export's dependencies are invisible to the project's tooling** — 266 MB
  under the skill, outside the root manifest and outside the firewall's
  dependency extract. A checkout that never runs the skill's own `npm install`
  has generation but no export.
- **Beyond ~20 nodes**, manual layout becomes painful. If the user has a
  bigger graph : suggest Mermaid + `File → Import → from Mermaid` in the
  desktop app.
- **Handwritten fonts** (Excalifont, Cascadia) are applied by the app on
  open. The JSON doesn't bundle the WOFF2 — `export.mjs` inlines them into
  the SVG itself, which is why an exported SVG is self-contained.
- **`index` field** : the app regenerates if invalid, but a valid sequence
  (`a0`, `a1`, …) avoids warnings.
- **No dark version of a file.** The theme is a reader preference, not a
  document property — there is no `theme` key, and a file authored in light
  colours already renders correctly in dark mode (the app inverts the whole
  canvas). A dark **image** is a render option, not a second source :
  `export.mjs` renders dark by default and `--light` turns it off, one source
  file for both. (The app's *Export image* dialog has the same toggle.)
  Full reasoning in [KNOWLEDGE L14](./KNOWLEDGE.md) — read it before
  agreeing to "make a dark variant".

---

## Smoke test (after creating the skill)

```bash
# Natural trigger : "fais-moi un diagramme excalidraw A → B → C in /tmp/test.excalidraw"
# → skill fires, Claude writes the file.

jq '.type, .version, (.elements | length)' /tmp/test.excalidraw
# expected : "excalidraw" \n 2 \n >= 7  (3 rects + 3 texts + 2 arrows)

# Open /tmp/test.excalidraw in excalidraw.com (drag & drop)
# - do arrows follow when you move a node ?
# - are the labels inside the boxes ?
# If yes, bindings OK.

# Export : natural trigger "exporte ce diagramme en png"
node .devcontainer/skills/diagram/scripts/export.mjs /tmp/test.excalidraw --out /tmp
# expected : "✓ test.svg  W×H   test@2x.png  2W×2H" — the PNG exactly twice the SVG

# fonts embedded — count varies with how many families the diagram uses
grep -c '@font-face' /tmp/test.svg     # expected : > 0
# Then READ /tmp/test@2x.png : no tofu boxes, dark background (#121212).
```
