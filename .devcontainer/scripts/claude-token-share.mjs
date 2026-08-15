#!/usr/bin/env -S node --experimental-eventsource
// shareglaude — user B side. Generates a PKCE verifier locally, asks
// shareglaude.app for a shortcode, prints the login URL to hand to user A,
// waits for the OAuth code over SSE, exchanges it against platform.claude.com,
// and writes ~/.claude/.credentials.json in the Claude Code shape.
//
// The code_verifier NEVER leaves this process — the shareglaude server sees
// only the challenge + state + eventual OAuth code, which by design cannot be
// converted into a token without the verifier.

import { randomBytes, createHash } from 'node:crypto';
import { readFile, writeFile, chmod, mkdir, rename, rm } from 'node:fs/promises';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { homedir, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { createInterface } from 'node:readline/promises';

const DEFAULT_BASE_URL = 'https://xxx.super.app';

const CLIENT_ID = '9d1c250a-e61b-44d9-88ed-5944d1962f5e';
const TOKEN_URL = 'https://platform.claude.com/v1/oauth/token';
const VALIDATE_URL = 'https://api.anthropic.com/api/oauth/validate';
const PROFILE_URL = 'https://api.anthropic.com/api/oauth/profile';
const ROLES_URL = 'https://api.anthropic.com/api/oauth/claude_cli/roles';
const SUBSCRIPTION_BY_ORG_TYPE = {
  claude_max: 'max',
  claude_pro: 'pro',
  claude_enterprise: 'enterprise',
  claude_team: 'team',
};
// Claude Code's fallback when the token response carries no
// refresh_token_expires_in — which, in practice, it never does.
const REFRESH_TOKEN_TTL_MS = 30 * 24 * 60 * 60 * 1000;
const REDIRECT_URI = 'https://platform.claude.com/oauth/code/callback';
const DEFAULT_CREDENTIALS_PATH = join(homedir(), '.claude', '.credentials.json');
// Same six scopes a plain `claude /login` requests, in the same order. Only a
// fallback: the token endpoint echoes the granted scopes back and those win.
const SCOPES = [
  'org:create_api_key',
  'user:profile',
  'user:inference',
  'user:sessions:claude_code',
  'user:mcp_servers',
  'user:file_upload',
];

// ---------- CLI parsing ----------

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.SHAREGLAUDE_URL || DEFAULT_BASE_URL,
    key: process.env.SHAREGLAUDE_KEY || '',
    dryRun: false,
    mockExchange: false,
    test: false,
    promote: false,
    credentialsPath: DEFAULT_CREDENTIALS_PATH,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--url') {
      args.baseUrl = argv[++i];
    } else if (a === '--key') {
      args.key = argv[++i];
    } else if (a === '--dry-run') {
      args.dryRun = true;
    } else if (a === '--test') {
      // Stage the token next to the live credentials instead of replacing them,
      // then offer to install it once validation has proven it works.
      args.test = true;
    } else if (a === '--promote') {
      args.promote = true;
    } else if (a === '--mock-exchange') {
      // Skip the real platform.claude.com token exchange and synthesise a fake
      // token payload. Intended for plumbing tests only — combine with --dry-run
      // to keep ~/.claude/.credentials.json untouched.
      args.mockExchange = true;
    } else if (a === '--credentials-path') {
      args.credentialsPath = argv[++i];
    } else if (a === '-h' || a === '--help') {
      printHelp();
      process.exit(0);
    } else {
      die(`Unknown argument: ${a}. Use --help for usage.`);
    }
  }
  // --promote is terminal: it installs an already-staged token and talks to
  // nothing. Combining it with the flow flags can only mean a mistaken command.
  if (args.promote && (args.test || args.dryRun || args.mockExchange)) {
    die('--promote runs alone: it installs an already-staged token, it does not run a session.');
  }
  if (!args.baseUrl) die('Base URL is empty. Pass --url or set SHAREGLAUDE_URL.');
  args.baseUrl = args.baseUrl.replace(/\/+$/, '');
  return args;
}

function stagedPathFor(credentialsPath) {
  return `${credentialsPath}.test`;
}

// The hint has to carry --credentials-path when it is not the default, or it
// would send the user to promote a staged file that does not exist.
function promoteHint(credentialsPath) {
  if (credentialsPath === DEFAULT_CREDENTIALS_PATH) {
    // Wrappers (the devcontainer's `wtf token share`) name their own follow-up
    // command here, so the hint matches how the user actually invoked us.
    return process.env.SHAREGLAUDE_PROMOTE_HINT || `${process.argv[1]} --promote`;
  }
  return `${process.argv[1]} --promote --credentials-path ${credentialsPath}`;
}

function printHelp() {
  console.log(`Usage: claude-token-share.mjs [options]

Options:
  --url <base>              Shareglaude base URL (default: ${DEFAULT_BASE_URL})
  --key <value>             Shared secret for POST /new (X-Shareglaude-Key header)
  --dry-run                 Print tokens to stdout, do not write credentials file
  --test                    Stage the token as <credentials>.test and leave the live
                            credentials untouched. Offers to install it right away
                            when stdin is a terminal.
  --promote                 Install a token previously staged by --test, then remove
                            the staged file. Runs alone; re-validates the staged
                            token and refuses one expired or revoked since staging.
  --mock-exchange           Skip the real Anthropic token exchange, synthesise fake
                            tokens (plumbing test only — combine with --dry-run)
  --credentials-path <path> Alternate credentials path (default: ~/.claude/.credentials.json)
  -h, --help                Show this help

Environment:
  SHAREGLAUDE_URL           Same effect as --url
  SHAREGLAUDE_KEY           Same effect as --key

Every real exchange is checked before anything is written, whatever the flags:
  - the token currently installed is reported, so you can see what you'd replace
  - the incoming token is validated (account, plan, org role, granted scopes)
  - claude -p runs against it in a throwaway config dir — the decisive proof
A token that fails any of this is never staged nor installed.

Notes:
  --dry-run wins over --test: it writes nothing at all, staged file included.
  --mock-exchange skips every check — synthetic tokens cannot be validated.
`);
}

function die(msg, code = 1) {
  console.error(`error: ${msg}`);
  process.exit(code);
}

// ---------- PKCE + state generation ----------

function b64url(buf) {
  return Buffer.from(buf).toString('base64url');
}

function generatePkce() {
  const verifier = b64url(randomBytes(32));
  const challenge = b64url(createHash('sha256').update(verifier).digest());
  const state = b64url(randomBytes(16));
  return { verifier, challenge, state };
}

// Best-effort git identity lookup — used only to personalise the greeting on
// the login page (falls back to "Someone" server-side if empty).
function readGitIdentity() {
  const read = (key) => {
    try {
      return execFileSync('git', ['config', '--get', key], {
        encoding: 'utf-8',
        stdio: ['ignore', 'pipe', 'ignore'],
      }).trim();
    } catch {
      return '';
    }
  };
  return { name: read('user.name'), email: read('user.email') };
}

// ---------- Server calls ----------

async function createSession(baseUrl, key, challenge, state) {
  const identity = readGitIdentity();
  const body = { challenge, state };
  if (identity.name) body.name = identity.name;
  if (identity.email) body.email = identity.email;
  const headers = { 'content-type': 'application/json' };
  if (key) headers['x-shareglaude-key'] = key;
  const res = await fetch(`${baseUrl}/new`, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
  if (res.status === 401) {
    die('shareglaude /new returned 401 — set SHAREGLAUDE_KEY (or pass --key <value>) to a key allowed by this server.');
  }
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    die(`shareglaude /new returned HTTP ${res.status}: ${body}`);
  }
  const data = await res.json();
  if (!data.sse_url || !data.login_url || !data.shortcode) {
    die(`shareglaude /new returned unexpected shape: ${JSON.stringify(data)}`);
  }
  return data;
}

function waitForShare(sseUrl, expectedState) {
  return new Promise((resolve, reject) => {
    const es = new EventSource(sseUrl);
    let resolved = false;

    const done = (fn, arg) => {
      if (resolved) return;
      resolved = true;
      try { es.close(); } catch {}
      fn(arg);
    };

    es.addEventListener('share', (ev) => {
      let payload;
      try {
        payload = JSON.parse(ev.data);
      } catch (e) {
        return done(reject, new Error(`Malformed share payload: ${e.message}`));
      }
      if (payload.state !== expectedState) {
        return done(
          reject,
          new Error(
            'State mismatch — server response rejected. Ask user A to redo the paste from a fresh browser tab.',
          ),
        );
      }
      if (typeof payload.code !== 'string' || !payload.code) {
        return done(reject, new Error('Share payload has no OAuth code.'));
      }
      done(resolve, payload.code);
    });

    es.onerror = () => {
      // EventSource fires onerror on both transient hiccups (readyState=CONNECTING)
      // and terminal closes (readyState=CLOSED). We only care about the terminal one.
      if (es.readyState === 2 /* CLOSED */) {
        done(
          reject,
          new Error(
            'Session dropped — the shareglaude connection closed. Re-run to get a new link.',
          ),
        );
      }
    };
  });
}

async function exchangeCode(code, verifier, state) {
  const body = {
    grant_type: 'authorization_code',
    code,
    state,
    code_verifier: verifier,
    client_id: CLIENT_ID,
    redirect_uri: REDIRECT_URI,
  };
  const res = await fetch(TOKEN_URL, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json' },
    body: JSON.stringify(body),
  });
  const text = await res.text();
  if (!res.ok) {
    die(`Token exchange failed: HTTP ${res.status} ${text}`);
  }
  let payload;
  try {
    payload = JSON.parse(text);
  } catch (e) {
    die(`Token endpoint returned non-JSON: ${text.slice(0, 200)}`);
  }
  if (!payload.access_token || !payload.refresh_token || !payload.expires_in) {
    die(`Token endpoint missing required fields: ${JSON.stringify(payload)}`);
  }
  return payload;
}

// ---------- Token inspection ----------

async function oauthCall(method, url, accessToken) {
  const res = await fetch(url, {
    method,
    headers: { authorization: `Bearer ${accessToken}`, 'content-type': 'application/json' },
  });
  const text = await res.text();
  if (!res.ok) throw new Error(`HTTP ${res.status} ${text.slice(0, 160)}`);
  return JSON.parse(text);
}

// /validate is the gate — the same check Claude Code runs on its own token, and
// the only one whose failure means the token is unusable. /profile and /roles
// are reporting: a hiccup there must not block an otherwise good token.
async function inspectToken(accessToken) {
  const validation = await oauthCall('POST', VALIDATE_URL, accessToken);
  const soft = async (url) => {
    try {
      return await oauthCall('GET', url, accessToken);
    } catch {
      return null;
    }
  };
  return { validation, profile: await soft(PROFILE_URL), roles: await soft(ROLES_URL) };
}

function printReport(label, { validation, profile, roles }, times) {
  const account = profile?.account;
  const org = profile?.organization;

  console.log(`  ${label}`);
  console.log(
    `    account : ${account ? `${account.full_name || account.display_name} <${account.email}>` : validation.account_uuid}`,
  );
  if (org) {
    const bits = [org.organization_type, org.rate_limit_tier, org.subscription_status]
      .filter(Boolean)
      .join(' · ');
    console.log(`    plan    : ${bits}`);
  }
  if (roles) {
    console.log(`    org     : ${roles.organization_name} · role ${roles.organization_role}`);
  }
  console.log(`    scopes  : ${(validation.scopes || []).join(' ')}`);
  if (times?.expiresAt) {
    console.log(`    access  : valid until ${new Date(times.expiresAt).toISOString()}`);
  }
  if (times?.refreshTokenExpiresAt) {
    // Assumed, not measured: the token endpoint does not return
    // refresh_token_expires_in, and using the refresh token to find out would
    // rotate it. Same 30-day assumption Claude Code makes.
    console.log(
      `    refresh : assumed valid until ${new Date(times.refreshTokenExpiresAt).toISOString()} (not server-confirmed)`,
    );
  }
}

// What is about to be replaced. Purely informational: a missing, expired or
// unreadable current token is a normal state, never a reason to stop.
async function printCurrentState(path) {
  console.log('');
  if (!existsSync(path)) {
    console.log(`  Current: none — ${path} does not exist yet.`);
    return;
  }

  let oauth;
  try {
    oauth = JSON.parse(await readFile(path, 'utf-8'))?.claudeAiOauth;
  } catch (e) {
    console.log(`  Current: ${path} is not readable JSON (${e.message}).`);
    return;
  }
  if (!oauth?.accessToken) {
    console.log(`  Current: ${path} holds no OAuth access token.`);
    return;
  }
  if (typeof oauth.expiresAt === 'number' && oauth.expiresAt <= Date.now()) {
    console.log(`  Current: access token expired on ${new Date(oauth.expiresAt).toISOString()}.`);
    console.log(`    scopes  : ${(oauth.scopes || []).join(' ')}`);
    return;
  }

  try {
    printReport('Current', await inspectToken(oauth.accessToken), oauth);
  } catch (e) {
    console.log(`  Current: token present but not usable (${e.message}).`);
  }
}

// The decisive check: Claude Code itself, on the token under test, in a
// throwaway config dir. CLAUDE_CODE_OAUTH_TOKEN is honoured as an auth source,
// and the empty config dir keeps the live credentials out of the picture.
function smokeClaudeCode(accessToken) {
  const dir = mkdtempSync(join(tmpdir(), 'shareglaude-'));
  const env = { ...process.env, CLAUDE_CONFIG_DIR: dir, CLAUDE_CODE_OAUTH_TOKEN: accessToken };
  // Either of these would authenticate instead, turning the check into a lie.
  delete env.ANTHROPIC_API_KEY;
  delete env.ANTHROPIC_AUTH_TOKEN;

  try {
    const out = execFileSync('claude', ['-p', 'Reply with exactly: ok'], {
      env,
      encoding: 'utf-8',
      timeout: 120000,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    console.log(`Claude Code accepted the token — replied ${JSON.stringify(out.trim().slice(0, 40))}.`);
  } catch (e) {
    if (e.code === 'ENOENT') {
      console.log('claude is not on PATH — skipped the end-to-end check.');
      return;
    }
    const detail = String(e.stderr || e.stdout || e.message).trim().split('\n')[0];
    die(`Claude Code refused the token: ${detail}`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// ---------- Credentials write ----------

// Mirrors what Claude Code writes after its own login, so an installed token is
// indistinguishable from one it obtained itself.
function buildCredentials(tokenPayload, profile) {
  const org = profile?.organization;
  const refreshExpiresIn = tokenPayload.refresh_token_expires_in;
  return {
    claudeAiOauth: {
      accessToken: tokenPayload.access_token,
      refreshToken: tokenPayload.refresh_token,
      expiresAt: Date.now() + Number(tokenPayload.expires_in) * 1000,
      refreshTokenExpiresAt:
        typeof refreshExpiresIn === 'number'
          ? Date.now() + refreshExpiresIn * 1000
          : Date.now() + REFRESH_TOKEN_TTL_MS,
      scopes:
        typeof tokenPayload.scope === 'string' && tokenPayload.scope.length > 0
          ? tokenPayload.scope.split(' ')
          : SCOPES.slice(),
      subscriptionType: SUBSCRIPTION_BY_ORG_TYPE[org?.organization_type] ?? null,
      rateLimitTier: org?.rate_limit_tier ?? null,
    },
  };
}

async function writeSecret(path, creds) {
  await mkdir(dirname(path), { recursive: true, mode: 0o700 });
  await writeFile(path, JSON.stringify(creds, null, 2), { mode: 0o600 });
  await chmod(path, 0o600);
}

// Replaces the live credentials, keeping the previous ones as a dated backup.
async function installCredentials(path, creds) {
  await mkdir(dirname(path), { recursive: true, mode: 0o700 });

  if (existsSync(path)) {
    const backup = `${path}.bak-${Date.now()}`;
    await rename(path, backup);
    console.log(`Existing credentials backed up to ${backup}`);
  }

  await writeSecret(path, creds);
  console.log(`Credentials written to ${path} (mode 0600).`);
}

async function promoteStaged(credentialsPath) {
  const staged = stagedPathFor(credentialsPath);
  if (!existsSync(staged)) {
    die(`No staged credentials at ${staged}. Run with --test first.`);
  }

  let creds;
  try {
    creds = JSON.parse(await readFile(staged, 'utf-8'));
  } catch (e) {
    die(`Staged credentials at ${staged} are not readable JSON: ${e.message}`);
  }

  const oauth = creds?.claudeAiOauth;
  const expiresAt = oauth?.expiresAt;
  if (typeof expiresAt === 'number' && expiresAt <= Date.now()) {
    die(
      `Staged token expired on ${new Date(expiresAt).toISOString()}. Delete ${staged} and re-run with --test.`,
    );
  }

  // Re-check rather than trust the staging run: time has passed, the token may
  // have been revoked, and a stale or synthetic staged file must never reach
  // the live credentials. A network failure is not the token's fault, so only
  // an actual answer from the API can block the install.
  if (oauth?.accessToken) {
    try {
      await oauthCall('POST', VALIDATE_URL, oauth.accessToken);
      console.log('Staged token re-validated.');
    } catch (e) {
      if (e.message.startsWith('HTTP')) {
        die(`Staged token is no longer valid (${e.message}). Delete ${staged} and re-run with --test.`);
      }
      console.log(`Could not reach the validation endpoint (${e.message}) — installing anyway.`);
    }
  }

  await installCredentials(credentialsPath, creds);
  await rm(staged);
  console.log(`Staged file ${staged} removed. Try: claude`);
}

async function confirm(question) {
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = await rl.question(question);
    return /^y(es)?$/i.test(answer.trim());
  } finally {
    rl.close();
  }
}

// ---------- Main ----------

async function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.promote) {
    await promoteStaged(args.credentialsPath);
    return;
  }

  const { verifier, challenge, state } = generatePkce();

  console.log(`Creating session on ${args.baseUrl} …`);
  const session = await createSession(args.baseUrl, args.key, challenge, state);

  console.log('');
  console.log('  Share this URL with the account holder (user A):');
  console.log('');
  console.log(`    ${session.login_url}`);
  console.log('');
  console.log('  Waiting for their paste over SSE. Ctrl+C cancels the session.');
  console.log('');

  const code = await waitForShare(session.sse_url, state);
  console.log('Received OAuth code. Exchanging for tokens …');

  const tokenPayload = args.mockExchange
    ? {
        access_token: `mock-access-${Date.now()}`,
        refresh_token: `mock-refresh-${Date.now()}`,
        expires_in: 3600,
        scope: SCOPES.join(' '),
      }
    : await exchangeCode(code, verifier, state);
  if (args.mockExchange) {
    console.log('[--mock-exchange] skipped real platform.claude.com call.');
  }
  console.log(
    `Token exchange OK (expires in ${tokenPayload.expires_in}s, scope: ${tokenPayload.scope || SCOPES.join(' ')}).`,
  );

  let report = null;
  if (args.mockExchange) {
    console.log('[--mock-exchange] token checks skipped — synthetic tokens cannot be validated.');
  } else {
    await printCurrentState(args.credentialsPath);
    try {
      report = await inspectToken(tokenPayload.access_token);
    } catch (e) {
      die(`Token validation failed: ${e.message}`);
    }
  }

  const creds = buildCredentials(tokenPayload, report?.profile);

  if (report) {
    console.log('');
    printReport('Incoming', report, creds.claudeAiOauth);
    console.log('');
    smokeClaudeCode(tokenPayload.access_token);
  }

  if (args.dryRun) {
    console.log('--- dry-run: credentials JSON (not written) ---');
    console.log(JSON.stringify(creds, null, 2));
    return;
  }

  if (args.test) {
    const staged = stagedPathFor(args.credentialsPath);
    await writeSecret(staged, creds);
    console.log(`Staged at ${staged} (mode 0600). ${args.credentialsPath} left untouched.`);

    // Piped or CI: prompting would hang forever, so hand back the command instead.
    if (!process.stdin.isTTY) {
      console.log(`To install it: ${promoteHint(args.credentialsPath)}`);
      return;
    }
    if (await confirm(`Install this token to ${args.credentialsPath}? [y/N] `)) {
      await promoteStaged(args.credentialsPath);
    } else {
      console.log(`Left staged. Install later with: ${promoteHint(args.credentialsPath)}`);
    }
    return;
  }

  await installCredentials(args.credentialsPath, creds);
  console.log('Token installed. Try: claude');
}

main().catch((e) => die(e.message || String(e)));
