#!/usr/bin/env node
// DOES THE UPDATE CHANNEL ACTUALLY RESOLVE?
//
// RG-83. Both Tauri configs point the updater at
// `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`.
// GitHub's `/releases/latest/` **excludes pre-releases**, and every Relay release
// so far is a pre-release — so the manifest resolves to nothing and returns 404.
//
// The updater is otherwise complete: built, wired, signed with a real minisign key,
// covered by tests. It resolves to nothing, and every installed copy is therefore
// un-updatable. That is invisible from the source, green in every test, and total
// in the field — which is exactly the class of failure that needs an instrument
// rather than a reader.
//
// This is that instrument. It reads the endpoint OUT OF THE CONFIG rather than
// restating it (a second copy of a URL is the next thing to drift), fetches it, and
// reports what a shipped copy of Relay would find there.
//
//   node scripts/check-updater.mjs          # report, exit 0 unless the channel is dead
//   node scripts/check-updater.mjs --strict # exit 1 on anything but a live manifest
//
// It is NOT in CI on every push, deliberately: until a non-prerelease is published
// the endpoint is legitimately 404, and a red build on every unrelated PR is a red
// build people learn to ignore. It belongs to the release ceremony — see
// docs/RELEASING.md — and to anyone asking "can a church actually receive a fix?".
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const strict = process.argv.includes('--strict');

/** Every endpoint any config configures, with the file that configured it. */
function endpoints() {
  const found = [];
  for (const rel of ['src-tauri/tauri.conf.json', 'src-tauri/tauri.updater.conf.json']) {
    let conf;
    try {
      conf = JSON.parse(readFileSync(resolve(root, rel), 'utf8'));
    } catch {
      continue;
    }
    for (const url of conf?.plugins?.updater?.endpoints ?? []) found.push([rel, url]);
  }
  return found;
}

const version = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8')).version;
const rows = endpoints();
if (!rows.length) {
  console.error('  ✗ no updater endpoint is configured anywhere');
  process.exit(1);
}

let dead = 0;
for (const [file, url] of rows) {
  // The updater substitutes these per platform. For a reachability check the
  // literal path is what matters, so they are only reported, never resolved.
  const templated = /\{\{/.test(url);
  let res;
  try {
    res = await fetch(url, { redirect: 'follow' });
  } catch (e) {
    dead += 1;
    console.error(`  ✗ ${url}\n      (${file}) unreachable — ${e.message}`);
    continue;
  }
  if (!res.ok) {
    dead += 1;
    console.error(`  ✗ ${url}\n      (${file}) HTTP ${res.status}${
      res.status === 404 && url.includes('/releases/latest/')
        ? ' — GitHub\'s /releases/latest/ excludes PRE-RELEASES. Publish a full release, or point at a tag.'
        : ''
    }`);
    continue;
  }
  let manifest;
  try {
    manifest = JSON.parse(await res.text());
  } catch {
    dead += 1;
    console.error(`  ✗ ${url}\n      (${file}) answered, but not with JSON`);
    continue;
  }
  const platforms = Object.keys(manifest.platforms ?? {});
  console.log(`  ✓ ${url}`);
  console.log(`      version ${manifest.version ?? '—'} · platforms: ${platforms.join(', ') || 'NONE'}`);
  if (!platforms.length) {
    dead += 1;
    console.error('      ✗ a manifest with no platforms updates nothing');
  }
  if (manifest.version && manifest.version === version) {
    console.log(`      note: same version as this checkout (${version}) — nothing would update`);
  }
  if (templated) console.log('      note: endpoint is templated; checked as written');
}

if (dead) {
  console.error(`\n  ${dead} of ${rows.length} update endpoints would not deliver a fix to an installed copy.`);
  process.exit(strict ? 1 : 0);
}
console.log(`\n  ${rows.length} update endpoint${rows.length === 1 ? '' : 's'} live.`);
