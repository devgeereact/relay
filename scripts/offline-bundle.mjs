#!/usr/bin/env node
// Assemble everything a church needs to install Relay with no internet at all.
//
//   node scripts/offline-bundle.mjs <installer> [<installer> …] --model <file> [--out DIR]
//
// ── The problem this solves ───────────────────────────────────────────────────
//
// Almost all of Relay already installs offline: the app is a single installer, the
// whole KJV is compiled into the binary, the templates and channels are seeded on
// first launch. One thing is not, and it is 148 MB — the speech model, which could
// only ever arrive over a connection the church does not have.
//
// So this makes a folder that fits on a USB stick: the installers, the model, and a
// README written for whoever is standing in the building rather than for whoever
// built it.
//
// ── What it verifies, and why it refuses rather than warns ───────────────────
//
// The model is checked against the SAME checksum the app checks, read out of
// `models.rs` so there is no second copy to drift. A wrong or truncated model does
// not fail loudly — whisper loads it and transcribes nonsense — and a bundle handed
// to a church is the worst possible place to discover that, because nobody will
// suspect the file. A bad checksum stops the build; it does not warn about it.

import { readFileSync, writeFileSync, mkdirSync, copyFileSync, statSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { basename, join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');

/** The catalogue, read from the Rust that ships — never re-typed here. */
function catalogue() {
  const src = readFileSync(join(ROOT, 'src-tauri/src/models.rs'), 'utf8');
  const block = src.slice(src.indexOf('const CATALOG'), src.indexOf('pub fn catalog'));
  const out = [];
  for (const m of block.matchAll(
    /filename:\s*"([^"]+)"[\s\S]*?label:\s*"([^"]+)"[\s\S]*?sha256:\s*"([0-9a-f]{64})"[\s\S]*?bytes:\s*([\d_]+)/g,
  )) {
    out.push({
      filename: m[1],
      label: m[2],
      sha256: m[3],
      bytes: Number(m[4].replace(/_/g, '')),
    });
  }
  if (!out.length) throw new Error('could not read the model catalogue from models.rs');
  return out;
}

function sha256(path) {
  const h = createHash('sha256');
  h.update(readFileSync(path));
  return h.digest('hex');
}

function die(msg) {
  console.error(`\n  ✗ ${msg}\n`);
  process.exit(1);
}

const argv = process.argv.slice(2);
const modelAt = argv.indexOf('--model');
const outAt = argv.indexOf('--out');
// `indexOf` returns -1 when a flag is absent, and -1 + 1 is 0 — which silently
// excluded the FIRST installer whenever `--out` was not given, i.e. always. The
// flag's value is only a value when the flag is there.
const valueAt = (flag) => (flag === -1 ? null : flag + 1);
const skip = new Set([valueAt(modelAt), valueAt(outAt)].filter((i) => i !== null));
const installers = argv.filter((a, i) => !a.startsWith('--') && !skip.has(i));
const modelPath = modelAt === -1 ? null : argv[modelAt + 1];
const outDir = resolve(ROOT, outAt === -1 ? 'offline-bundle' : argv[outAt + 1]);

if (!installers.length || !modelPath) {
  console.error(
    '\nusage: node scripts/offline-bundle.mjs <installer…> --model <ggml-*.bin> [--out DIR]\n',
  );
  process.exit(2);
}

for (const f of [...installers, modelPath]) if (!existsSync(f)) die(`${f} does not exist`);

// ── Verify the model BEFORE anything is copied ──────────────────────────────
const size = statSync(modelPath).size;
const candidates = catalogue().filter((m) => m.bytes === size);
if (!candidates.length) {
  die(
    `${basename(modelPath)} is ${size} bytes, which is not the size of any model Relay knows.\n` +
      `    Known: ${catalogue().map((m) => `${m.filename} (${m.bytes})`).join(', ')}`,
  );
}
const digest = sha256(modelPath);
const model = candidates.find((m) => m.sha256 === digest);
if (!model) {
  // A refusal, not a warning. A church cannot check this and will not suspect it.
  die(
    `${basename(modelPath)} did not match its checksum.\n` +
      `    expected ${candidates[0].sha256}\n    got      ${digest}\n` +
      '    A truncated model does not fail loudly — whisper loads it and transcribes nonsense.',
  );
}

mkdirSync(outDir, { recursive: true });
for (const f of installers) copyFileSync(f, join(outDir, basename(f)));
copyFileSync(modelPath, join(outDir, model.filename));

const version = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8')).version;

writeFileSync(
  join(outDir, 'READ ME FIRST.txt'),
  `RELAY ${version} — installing with no internet
${'='.repeat(46)}

This folder has everything. Nothing here needs a connection.

1. INSTALL THE APP
   Run the installer for your computer:
${installers.map((f) => `     ${basename(f)}`).join('\n')}

2. COPY THE SPEECH MODEL
   Copy this file into your Downloads folder:

     ${model.filename}

   That is the ${model.label.toLowerCase()} model. It is large because it does the
   listening, and it stays on your computer — Relay never sends anything anywhere.

3. OPEN RELAY
   Go to Settings, then Network. Under the list of models you will see
   "Found on this computer" with an Install button. Press it.

   Relay checks the file is exactly the one it expects before it uses it, so if
   the copy went wrong it will tell you rather than mishearing everything.

4. THAT IS ALL
   The whole Bible is already inside the app. So are the screen designs. Relay
   works completely offline from here — the only thing it will ever want the
   internet for is an update, and it will not ask during a service.

If something is wrong: Settings → Diagnostics → "Save a diagnostic file", and send
that. It contains no recordings, no transcripts and nothing anybody said.
`,
);

const bytes = (n) => `${(n / 1e6).toFixed(0)} MB`;
console.log(`
  ✓ Offline bundle ready — ${outDir}

    ${installers.map((f) => `${basename(f)} (${bytes(statSync(f).size)})`).join('\n    ')}
    ${model.filename} (${bytes(model.bytes)}) — checksum verified
    READ ME FIRST.txt

  Copy the whole folder to a USB stick.
`);
