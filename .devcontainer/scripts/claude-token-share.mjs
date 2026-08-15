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
import { existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { createInterface } from 'node:readline/promises';

const DEFAULT_BASE_URL = 'https://xxx.super.app';

const CLIENT_ID = '9d1c250a-e61b-44d9-88ed-5944d1962f5e';
const TOKEN_URL = 'https://platform.claude.com/v1/oauth/token';
const VALIDATE_URL = 'https://api.anthropic.com/api/oauth/validate';
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
  const suffix =
    credentialsPath === DEFAULT_CREDENTIALS_PATH ? '' : ` --credentials-path ${credentialsPath}`;
  return `${process.argv[1]} --promote${suffix}`;
}

function printHelp() {
  console.log(`Usage: claude-token-share.mjs [options]

Options:
  --url <base>              Shareglaude base URL (default: ${DEFAULT_BASE_URL})
  --key <value>             Shared secret for POST /new (X-Shareglaude-Key header)
  --dry-run                 Print tokens to stdout, do not write credentials file
  --test                    Validate the token, stage it as <credentials>.test and
                            leave the live credentials untouched. Offers to install
                            it right away when stdin is a terminal.
  --promote                 Install a token previously staged by --test, then remove
                            the staged file. Runs alone, offline.
  --mock-exchange           Skip the real Anthropic token exchange, synthesise fake
                            tokens (plumbing test only — combine with --dry-run)
  --credentials-path <path> Alternate credentials path (default: ~/.claude/.credentials.json)
  -h, --help                Show this help

Environment:
  SHAREGLAUDE_URL           Same effect as --url
  SHAREGLAUDE_KEY           Same effect as --key

Notes:
  --dry-run wins over --test: it writes nothing at all, staged file included.
  --mock-exchange skips validation — synthetic tokens cannot be validated.
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

// Same check Claude Code runs on its own token. A 200 here is the only proof
// that the grant is usable — an exchange returning 200 says nothing about
// whether the scopes actually cover anything.
async function validateToken(accessToken) {
  const res = await fetch(VALIDATE_URL, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
  });
  const text = await res.text();
  if (!res.ok) {
    die(`Token validation failed: HTTP ${res.status} ${text.slice(0, 200)}`);
  }
  try {
    return JSON.parse(text);
  } catch {
    die(`Token validation returned non-JSON: ${text.slice(0, 200)}`);
  }
}

// ---------- Credentials write ----------

function buildCredentials(tokenPayload) {
  return {
    claudeAiOauth: {
      accessToken: tokenPayload.access_token,
      refreshToken: tokenPayload.refresh_token,
      expiresAt: Date.now() + Number(tokenPayload.expires_in) * 1000,
      scopes:
        typeof tokenPayload.scope === 'string' && tokenPayload.scope.length > 0
          ? tokenPayload.scope.split(' ')
          : SCOPES.slice(),
      subscriptionType: null,
      rateLimitTier: null,
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

  const expiresAt = creds?.claudeAiOauth?.expiresAt;
  if (typeof expiresAt === 'number' && expiresAt <= Date.now()) {
    die(
      `Staged token expired on ${new Date(expiresAt).toISOString()}. Delete ${staged} and re-run with --test.`,
    );
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

  if (args.mockExchange) {
    console.log('[--mock-exchange] token validation skipped — synthetic tokens cannot be validated.');
  } else {
    const account = await validateToken(tokenPayload.access_token);
    const email = account?.account?.email;
    console.log(`Token validated${email ? ` — account ${email}` : ''}.`);
  }

  const creds = buildCredentials(tokenPayload);

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
