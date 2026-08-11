#!/usr/bin/env node
// The single place Relay's version number is read or written.
//
// Why this file exists: the version lived in THREE files that nothing kept in
// agreement, and nothing in CI ever looked at them.
//
//   src-tauri/tauri.conf.json   ← what the updater manifest advertises
//   package.json                ← npm
//   src-tauri/Cargo.toml        ← the crate
//
// All three said 0.1.0. The release workflow never read any of them, and never
// compared them to the tag. So `git tag v0.2.0 && git push` produced a release
// whose latest.json advertised the new artifacts *under version 0.1.0* — and every
// installed copy of Relay compared that against its own 0.1.0, decided it was
// already up to date, and never updated. Silently. Forever.
//
// That is the exact failure the updater exists to prevent: we fix a bug that took
// down a service, we ship it, and the church never receives it. There is no error
// anywhere. The only symptom is a fix that never arrives.
//
// So: one script owns all three, CI asserts they agree on every PR, and the release
// gate asserts they also equal the tag before it builds anything.
//
//   node scripts/version.mjs --check            all three agree
//   node scripts/version.mjs --check 0.2.0      all three agree AND equal 0.2.0
//   node scripts/version.mjs --set   0.2.0      write all three

import { readFileSync, writeFileSync } from 'node:fs';

const TAURI = 'src-tauri/tauri.conf.json';
const NPM = 'package.json';
const CARGO = 'src-tauri/Cargo.toml';

// Cargo's [package] version — the FIRST `version = "…"` at the start of a line.
// Dependency versions are indented or inline, so they can't match.
const CARGO_VERSION = /^version\s*=\s*"([^"]+)"/m;

const read = (p) => readFileSync(p, 'utf8');

function current() {
  return {
    [TAURI]: JSON.parse(read(TAURI)).version,
    [NPM]: JSON.parse(read(NPM)).version,
    [CARGO]: read(CARGO).match(CARGO_VERSION)?.[1],
  };
}

// Semver, with an optional pre-release tail — and the tail must be NUMERIC.
//
// Tauri compares versions as semver to decide whether an update is newer, so a
// version it cannot parse is a version no church ever updates past. But the Windows
// MSI target is stricter than semver, and it does not tell you so until it has
// compiled the whole application:
//
//   failed to bundle project: `optional pre-release identifier in app version must
//   be numeric-only and cannot be greater than 65535 for msi target`
//
// So `0.1.0-rc4` builds a perfectly good .dmg and then dies on the MSI, fifteen
// minutes into a release, on the platform most of our churches are on. And it cannot
// be avoided by simply not using pre-releases: release.yml REQUIRES a hyphenated
// (pre-release) tag for any unsigned build, and the tag must equal this version.
//
// Pre-releases are therefore numbered, not named: 0.1.0-1, 0.1.0-2. Uglier than
// "rc4", and it fails here in a second rather than there in a quarter of an hour.
const SEMVER = /^\d+\.\d+\.\d+(?:-\d+)?$/;
const MSI_MAX_PRERELEASE = 65535;

const BAD_VERSION_HELP =
  'Use 1.2.3, or 1.2.3-4 for a pre-release.\n\n' +
  '      The pre-release identifier must be a NUMBER, not a name. "0.1.0-rc4" is valid\n' +
  '      semver and builds a working .dmg — and then the Windows MSI bundler rejects it,\n' +
  '      fifteen minutes into the release, with "pre-release identifier ... must be\n' +
  '      numeric-only". Windows is the platform most of our churches are on.';

function fail(msg, hint) {
  console.error(`\n  ✗ ${msg}`);
  if (hint) console.error(`\n  ${hint}`);
  console.error('');
  process.exit(1);
}

// ONE validator, used by both --set and --check.
//
// These files are also hand-edited and merged, so --check is not merely a
// re-run of what --set already allowed: it is the last gate before the release
// workflow starts building. It used to be the weaker of the two — it tested the
// shape but not the MSI's 65535 ceiling on the pre-release number, so a version
// --set would have refused could still reach a tag and die in the Windows
// bundler a quarter of an hour later. That is the exact failure this file exists
// to move forward in time.
function validate(v, where) {
  if (!SEMVER.test(v)) fail(`${where} has an unusable version: "${v}"`, BAD_VERSION_HELP);
  const pre = v.split('-')[1];
  if (pre && Number(pre) > MSI_MAX_PRERELEASE) {
    fail(
      `${where} has pre-release number ${pre}, above the MSI limit of ${MSI_MAX_PRERELEASE}.`,
      BAD_VERSION_HELP,
    );
  }
}

function check(expected) {
  const found = current();
  const values = Object.values(found);

  for (const [file, v] of Object.entries(found)) {
    if (!v) fail(`No version found in ${file}.`);
    validate(v, file);
  }
  if (expected) validate(expected, 'The tag');

  if (new Set(values).size !== 1) {
    const list = Object.entries(found)
      .map(([f, v]) => `      ${v.padEnd(12)} ${f}`)
      .join('\n');
    fail(
      "Relay's version files disagree:\n\n" + list,
      'Fix with:  npm run version:set -- <version>',
    );
  }

  if (expected && values[0] !== expected) {
    fail(
      `The tag says ${expected}, but the repo says ${values[0]}.\n\n` +
        '      An updater manifest built from this would advertise the new build under the\n' +
        '      OLD version number. Every existing install would compare it as "same version"\n' +
        '      and never update — which is the one thing the updater exists to prevent.',
      `Fix with:  npm run version:set -- ${expected}\n  ` +
        `           git commit -am "chore(release): ${expected}" && git push\n  ` +
        `           git tag -f v${expected} && git push -f origin v${expected}`,
    );
  }

  console.log(`  ✓ version ${values[0]} — consistent across all three files`);
}

function set(v) {
  validate(v, `"${v}"`);

  const tauri = JSON.parse(read(TAURI));
  tauri.version = v;
  writeFileSync(TAURI, JSON.stringify(tauri, null, 2) + '\n');

  const npm = JSON.parse(read(NPM));
  npm.version = v;
  writeFileSync(NPM, JSON.stringify(npm, null, 2) + '\n');

  const cargo = read(CARGO);
  if (!CARGO_VERSION.test(cargo)) fail(`Could not find a [package] version in ${CARGO}.`);
  writeFileSync(CARGO, cargo.replace(CARGO_VERSION, `version = "${v}"`));

  console.log(`  ✓ set version ${v} in all three files`);
  console.log('    Commit this before you tag — the release gate compares the tag to the repo.');
}

const [mode, arg] = process.argv.slice(2);
if (mode === '--check') check(arg ? arg.replace(/^v/, '') : null);
else if (mode === '--set' && arg) set(arg.replace(/^v/, ''));
else fail('Usage: version.mjs --check [version] | --set <version>');
