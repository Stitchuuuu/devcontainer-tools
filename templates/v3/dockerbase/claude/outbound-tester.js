#!/usr/bin/env node
/**
 * outbound-tester — CLI for driving the outbound control channel from the
 * host / a script. Companion to the vscode-ext-patchs/{outbound-action-injector,
 * webview-simulated-click}.py patches.
 *
 * Reads .devcontainer/logs/claude-code-vscode-ext-pending-perms.jsonl (written
 * by the extension's instrumented sendRequest) to discover the requestId of
 * each currently-pending tool_permission_request. Writes injection commands
 * to .devcontainer/logs/claude-code-vscode-ext-outbound.jsonl for the
 * extension's file watcher to pick up (200 ms poll).
 *
 * Subcommands
 * -----------
 *   list [--json]
 *       Print unsettled perm requests (grouped by requestId, latest state
 *       wins). Default: tabular. --json: machine-readable.
 *
 *   send <requestId> allow [--input '<json>'] [--sid <sessionId>]
 *   send <requestId> deny  [--message '<text>']  [--sid <sessionId>]
 *       Append a tool_permission_response line to outbound.jsonl. Auto-
 *       resolves sessionId from pending-perms.jsonl unless --sid is given.
 *       --input overrides the inputs to send (deep merge preserves fields
 *       not specified). If omitted, echoes back the original inputs.
 *
 * Exit codes
 * ----------
 *   0 : success
 *   1 : argparse error / requestId not found (for `send`)
 *   2 : IO error
 */

"use strict";
const fs = require("fs");
const path = require("path");

const LOGS_DIR = path.resolve(__dirname, "..", "logs");
const PENDING = path.join(LOGS_DIR, "claude-code-vscode-ext-pending-perms.jsonl");
const OUTBOUND = path.join(LOGS_DIR, "claude-code-vscode-ext-outbound.jsonl");

function die(code, msg) {
    console.error(msg);
    process.exit(code);
}

function readJsonl(file) {
    // Missing file is not fatal — treat as empty. This is the boot state
    // before any perm request has been sent.
    if (!fs.existsSync(file)) return [];
    const raw = fs.readFileSync(file, "utf8");
    const records = [];
    for (const line of raw.split("\n")) {
        const t = line.trim();
        if (!t) continue;
        try { records.push(JSON.parse(t)); }
        catch (e) { console.warn(`[outbound-tester] skip bad line in ${path.basename(file)}: ${e.message}`); }
    }
    return records;
}

function aggregatePending() {
    // pending-perms.jsonl is append-only :
    //   - perm records: {ts, sessionId, channelId, requestId, ...}
    //     first has settled:false + full metadata ; second has settled:true.
    //   - boot markers: {ts, ev:"session_boot"} written by the ext watcher
    //     at each extension host init. On reload, the SDK aborts pending
    //     Gn instances but our settle-log injection only hooks resolve —
    //     reject/abort never writes settled:true, so pre-boot records
    //     would linger as ghosts. We use the latest boot marker's ts as
    //     the cutoff.
    const records = readJsonl(PENDING);
    let bootTs = null;
    for (const rec of records) {
        if (rec.ev === "session_boot" && rec.ts && (!bootTs || rec.ts > bootTs)) {
            bootTs = rec.ts;
        }
    }
    // Fallback if no boot marker (fresh file, log rotation) : records
    // older than 30 min are treated as fossilized.
    const fallbackCutoff = new Date(Date.now() - 30 * 60 * 1000).toISOString();
    const cutoff = bootTs || fallbackCutoff;

    const byRid = {};
    for (const rec of records) {
        if (rec.ev) continue;                    // skip boundary markers
        if (!rec.requestId) continue;
        if (rec.ts && rec.ts < cutoff) continue; // stale — skip
        byRid[rec.requestId] = { ...byRid[rec.requestId], ...rec };
    }
    const pending = [];
    for (const rid in byRid) {
        if (byRid[rid].settled === false) pending.push(byRid[rid]);
    }
    return pending;
}

function resolvePending(token) {
    // Resolve a positional token to a single pending record.
    //   - Exact match on requestId (full 32-char hex) wins immediately.
    //   - Otherwise prefix-match against requestId, sessionId, and
    //     channelId across all pending. Unique hit required — ambiguity
    //     is a fail-loud condition (user must disambiguate).
    const pending = aggregatePending();
    if (pending.length === 0) return { pending, matches: [] };
    const exact = pending.find(p => p.requestId === token);
    if (exact) return { pending, matches: [exact] };
    const matches = pending.filter(p =>
        (p.requestId && p.requestId.startsWith(token)) ||
        (p.sessionId && p.sessionId.startsWith(token)) ||
        (p.channelId && p.channelId.startsWith(token))
    );
    return { pending, matches };
}

function fmtAge(iso) {
    const ms = Date.now() - new Date(iso).getTime();
    if (!isFinite(ms)) return "?";
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    if (s < 3600) return `${Math.round(s / 60)}m`;
    return `${Math.round(s / 3600)}h`;
}

function truncate(s, n) {
    if (typeof s !== "string") s = JSON.stringify(s ?? "");
    return s.length > n ? s.slice(0, n - 1) + "…" : s;
}

function cmdList(args) {
    const asJson = args.includes("--json");
    const pending = aggregatePending();
    if (asJson) {
        console.log(JSON.stringify(pending, null, 2));
        return 0;
    }
    if (pending.length === 0) {
        console.log("(no pending perm requests)");
        return 0;
    }
    // Tabular : sid8  chan   rid8  focus  tool  inputs  age
    // `focus` = one of "F" (focused), "A" (active but not focused), "-" (neither),
    // "?" (unknown / pre-boot record with no focus data).
    console.log(`${"sid8".padEnd(9)}${"chan".padEnd(8)}${"rid8".padEnd(10)}${"foc".padEnd(5)}${"tool".padEnd(14)}${"inputs".padEnd(43)}age`);
    console.log("─".repeat(96));
    for (const p of pending) {
        const sid8 = (p.sessionId || "").slice(0, 8);
        const chan = (p.channelId || "").slice(0, 6);
        const rid8 = (p.requestId || "").slice(0, 8);
        const foc = p.focused === true ? "F"
                  : p.focused === false && p.active === true ? "A"
                  : p.focused === false ? "-"
                  : "?";
        const tool = truncate(p.toolName || "", 12);
        const inputs = truncate(JSON.stringify(p.inputs ?? {}), 41);
        const age = fmtAge(p.ts);
        console.log(`${sid8.padEnd(9)}${chan.padEnd(8)}${rid8.padEnd(10)}${foc.padEnd(5)}${tool.padEnd(14)}${inputs.padEnd(43)}${age}`);
    }
    return 0;
}

function parseFlags(argv) {
    // Strip and collect --flag <value> pairs into a plain object ; return
    // (positional, flags). Bare --json is treated as {json:true}.
    const flags = {};
    const positional = [];
    for (let i = 0; i < argv.length; i++) {
        const a = argv[i];
        if (a.startsWith("--")) {
            const key = a.slice(2);
            const next = argv[i + 1];
            if (next !== undefined && !next.startsWith("--")) {
                flags[key] = next;
                i++;
            } else {
                flags[key] = true;
            }
        } else {
            positional.push(a);
        }
    }
    return { positional, flags };
}

function cmdSend(args) {
    const { positional, flags } = parseFlags(args);
    if (positional.length < 2) die(1, "usage: send <sid8|chan|rid8|full-rid> allow|deny [--input '<json>'] [--message '<text>'] [--sid <sessionId>]");
    const [token, behavior] = positional;
    if (behavior !== "allow" && behavior !== "deny") die(1, `behavior must be 'allow' or 'deny' (got ${behavior})`);

    // Prefix-match the positional against pending (rid/sid/chan). Bypass
    // when --sid is passed AND the token looks like a full requestId — user
    // knows what they're doing.
    let sessionId = flags.sid;
    let requestId;
    let inputsFromPending;

    const fullRid = /^[0-9a-f]{32}$/i.test(token);
    if (sessionId && fullRid) {
        // Force path — treat token as a full requestId, don't try to
        // resolve. Still try to echo inputs from pending if present.
        requestId = token;
        inputsFromPending = aggregatePending().find(p => p.requestId === token)?.inputs;
    } else {
        const { pending, matches } = resolvePending(token);
        if (matches.length === 0) {
            const hint = pending.length
                ? `pending: ${pending.map(p => `${(p.sessionId||"").slice(0,8)}/${(p.requestId||"").slice(0,8)}`).join(", ")}`
                : "(list is empty)";
            die(1, `no pending match for "${token}" — ${hint}`);
        }
        if (matches.length > 1) {
            const listed = matches.map(p => `sid8=${(p.sessionId||"").slice(0,8)} rid=${p.requestId}`).join("\n  ");
            die(1, `ambiguous token "${token}" — ${matches.length} matches:\n  ${listed}`);
        }
        const hit = matches[0];
        requestId = hit.requestId;
        if (!sessionId) sessionId = hit.sessionId;
        inputsFromPending = hit.inputs;
    }

    if (!sessionId) die(1, `sessionId could not be resolved and --sid was not provided`);

    const cmd = { cmd: "tool_permission_response", sessionId, requestId, behavior };

    if (behavior === "allow") {
        if (flags.input) {
            try { cmd.updatedInput = JSON.parse(flags.input); }
            catch (e) { die(1, `--input must be valid JSON: ${e.message}`); }
        } else {
            cmd.updatedInput = inputsFromPending ?? {};
        }
        cmd.updatedPermissions = [];
    } else {
        cmd.message = flags.message ?? "denied";
    }

    // Ensure the directory + file exist ; extension.js also creates them
    // defensively, but we may run before the ext has started.
    try { fs.mkdirSync(LOGS_DIR, { recursive: true }); } catch (_) {}
    fs.appendFileSync(OUTBOUND, JSON.stringify(cmd) + "\n");
    console.log(`✓ appended to ${path.relative(process.cwd(), OUTBOUND)}: ${behavior} sid=${sessionId.slice(0, 8)} rid=${requestId}`);
    return 0;
}

function main() {
    const argv = process.argv.slice(2);
    const [subcmd, ...rest] = argv;
    switch (subcmd) {
        case "list":
            process.exit(cmdList(rest));
        case "send":
            process.exit(cmdSend(rest));
        default:
            console.error(
`outbound-tester — drive Claude Code perm requests from CLI

Usage :
  outbound-tester.js list [--json]
  outbound-tester.js send <requestId> allow [--input '<json>'] [--sid <sessionId>]
  outbound-tester.js send <requestId> deny  [--message '<text>']  [--sid <sessionId>]

Files :
  ${path.relative(process.cwd(), PENDING)}   (read — populated by ext.js instrumented sendRequest)
  ${path.relative(process.cwd(), OUTBOUND)}  (write — polled by ext.js watcher, 200 ms)
`);
            process.exit(subcmd ? 1 : 0);
    }
}

main();
