Run the Claude Code context audit of this project with the kit in claude-context-audit/. First read the example claude-context-audit/exemple/rapport.md: your report must have exactly the same shape. The script's options are described at the top of claude-context-audit/audit.mjs.

1. Pick the project's usual name, the way it is written in a sentence (for example "DevContainer Tools" rather than "devcontainer-tools"). Then run, from the project root:
   node claude-context-audit/audit.mjs --name="<project name>"
   If the script can't find the transcripts folder, it lists the ones that exist: run it again with the right folder as the last argument.

2. The script writes to claude-context-audit/<project-name>/: donnees.md, context-table.excalidraw and context-chart.excalidraw. Export both .excalidraw files with the export script of the /diagram skill:
   - <name>.png: default dark render, rename the @2x.png to <name>.png;
   - <name>-dark.svg: dark render;
   - <name>.svg: light render (--light --svg-only).
   Open both PNGs and check that no text overflows or overlaps. If export isn't available, say so and keep the .excalidraw files.

3. Write claude-context-audit/<project-name>/rapport.md, modelled on the example, in English: translate its section titles and fixed texts. Keep the sections in the same order:
   - title and intro sentence with the project name, the number of sessions and the period (in donnees.md);
   - the chart in a <picture> (light and dark SVG), then links to the PNGs and the .excalidraw files;
   - "What we measure": translate the example's text;
   - "The numbers": the table from donnees.md, translated, then the thresholds;
   - "The 10 sessions with the largest max context": from the ranking in donnees.md, without session ids. Rewrite each topic as a few generic words, with no client, person or internal product name;
   - "What it shows": findings specific to this project, ranked by estimated gain. Each one rests on a number from donnees.md. For the 3 largest sessions, open the transcript and say what made the context grow: topics chained without /clear, no /compact past 200k, large files or tool outputs read in the main thread, exploration that should have gone to a subagent;
   - "Method": translate the example's text.
   No cumulative token totals: only the context each call re-reads.
   The two images keep their French labels; say so at the end of your answer.

Change nothing in the project outside claude-context-audit/<project-name>/.
