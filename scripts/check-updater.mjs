#!/usr/bin/env node
// DOES THE UPDATE CHANNEL ACTUALLY RESOLVE?
//
// RG-83. Both Tauri configs used to point the updater at
// `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`.
// GitHub's `/releases/latest/` **excludes pre-releases**, and every Relay release
// so far is a pre-release — so the manifest resolved to nothing and returned 404,
// for months. The updater was otherwise complete: built, wired, signed with a real
// minisign key, covered by tests. It resolved to nothing, and every installed copy
// was therefore un-updatable. Invisible from the source, green in every test, and
// total in the field — exactly the class of failure that needs an instrument
// rather than a reader.
//
// **The first repair was wrong in a way that looked right.** The endpoint was
// pinned at each release's own tag — `…/releases/download/v0.2.0-1/latest.json` —
// which resolves, so this script went green. It does not work: a build asks its
// OWN release's manifest, is told its own version, and concludes it is up to date
// for ever. The next release lives at a URL it has never heard of. The symptom was
// printed by this very script, as `note: same version as this checkout — nothing
// would update`, and read as benign.
//
// The endpoint is now a CONSTANT: a permanent release tagged `updates` whose only
// asset is `latest.json`, re-uploaded by `update-channel-promote.yml` whenever a
// release is published. Every build ever shipped asks the same address, and that
// address always describes the newest release. `scripts/version.mjs --check` — a
// CI gate on every PR — refuses both drifts: back to `/latest/`, and forward to a
// version tag.
//
// This is that instrument. It reads the endpoint OUT OF THE CONFIG rather than
// restating it (a second copy of a URL is the next thing to drift), fetches it, and
// reports what a shipped copy of Relay would find there.
//
//   node scripts/check-updater.mjs          # report, exit 0 unless the channel is dead
//   node scripts/check-updater.mjs --strict # exit 1 on anything but a live manifest
//
// It is NOT in CI on every push, deliberately: the channel describes the newest
// PUBLISHED release, so on a branch that has bumped the version it legitimately
// reports an older one, and until the first release is published after this change
// it reports nothing at all. A red build on every unrelated PR is a red build
// people learn to ignore. It belongs to the release ceremony — see
// docs/RELEASING.md — and to anyone asking "can a church actually receive a fix?".
// The **Update channel** workflow runs it on demand from the Actions tab.
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
    for (const url of conf?.plugins?.updater?.endpoints ?? []) {
      found.push([rel, url]);
    }
  }
  return found;
}

const version = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8')).version;
const rows = endpoints();
if (!rows.length) {
  console.error('  ✗ no updater endpoint is configured anywhere');
  process.exit(1);
}

/**
 * Is this an endpoint we are willing to FETCH?
 *
 * The URL comes out of a config file, and this script then makes a network
 * request to it — which CodeQL flags as `js/file-access-to-http`, correctly as a
 * shape even though the file is one of ours and is tracked. The right answer is
 * not to silence it: **an updater endpoint that is not an HTTPS GitHub release
 * URL is itself a finding**, and a checker that would happily fetch whatever the
 * config named would be the wrong instrument for noticing it.
 *
 * So the shape is asserted before anything is fetched, and a URL that fails it is
 * reported as a failure rather than followed.
 */
function permitted(url) {
  let u;
  try {
    u = new URL(url);
  } catch {
    return 'not a URL at all';
  }
  if (u.protocol !== 'https:') return `not HTTPS (${u.protocol})`;
  if (u.hostname !== 'github.com' && u.hostname !== 'api.github.com') {
    return `not a GitHub host (${u.hostname})`;
  }
  return null;
}

let dead = 0;
for (const [file, url] of rows) {
  const refused = permitted(url);
  if (refused) {
    dead += 1;
    console.error(
      `  ✗ ${url}\n      (${file}) ${refused} — this is where a shipped copy of Relay ` +
        `would look for its updates. Not fetched.`,
    );
    continue;
  }
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
        ? " — GitHub's /releases/latest/ excludes PRE-RELEASES. The endpoint should be the `updates` channel release; see update-channel-promote.yml."
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
