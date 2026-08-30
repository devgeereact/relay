// RG-19 — installing Relay with no internet, and the pack that was NOT built.
//
// Almost all of Relay already installs offline: the app is a single installer, the
// whole KJV is compiled into the binary, the templates are seeded on first launch.
// One thing was not, and it is 148 MB — the speech model could only ever arrive over
// a connection the church does not have. For this market that is not an edge case;
// it is a reason a church cannot use Relay at all.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const store = await import('./stores/capture.js');
const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

beforeEach(() => {
  invoke.mockReset();
  store.capture.update((s) => ({ ...s, available: true }));
});

describe('the store wrappers', () => {
  it('a failed scan shows "none found" rather than taking the screen down', async () => {
    invoke.mockRejectedValue(new Error('no backend'));
    await expect(store.findModelFiles()).resolves.toEqual([]);
  });

  it('installing THROWS on failure — a silent one leaves the operator believing they have a model', async () => {
    invoke.mockRejectedValue(new Error('did not match its checksum'));
    await expect(store.installModelFile('/x/ggml-base.bin')).rejects.toThrow();
  });

  it('refreshes the model list after installing, so the UI cannot lag the truth', async () => {
    const calls = [];
    invoke.mockImplementation(async (cmd) => {
      calls.push(cmd);
      return cmd === 'list_models' ? [] : 'base';
    });
    await store.installModelFile('/x/ggml-base.bin');
    expect(calls).toEqual(['install_model_file', 'list_models']);
  });
});

describe('the bundle script', () => {
  const run = (args) => {
    try {
      return {
        code: 0,
        out: execFileSync('node', ['scripts/offline-bundle.mjs', ...args], {
          cwd: ROOT,
          encoding: 'utf8',
          stdio: ['ignore', 'pipe', 'pipe'],
        }),
      };
    } catch (e) {
      return { code: e.status, out: `${e.stdout ?? ''}${e.stderr ?? ''}` };
    }
  };

  it('reads the catalogue out of the Rust that ships, so there is no second copy', () => {
    // A hardcoded checksum here would be a second source of truth, and the one that
    // drifts is always the copy nobody runs.
    const src = read('scripts/offline-bundle.mjs');
    expect(src).toMatch(/src-tauri\/src\/models\.rs/);
    expect(src).not.toMatch(/60ed5bc3dd14eea856493d334349b405782ddcaf/);
  });

  it('REFUSES a file that is not a model Relay knows, and names the sizes it wants', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'relay-ob-'));
    fs.writeFileSync(path.join(dir, 'Relay.dmg'), 'installer');
    fs.writeFileSync(path.join(dir, 'ggml-base.bin'), 'not a model');
    const r = run([path.join(dir, 'Relay.dmg'), '--model', path.join(dir, 'ggml-base.bin')]);
    expect(r.code).toBe(1);
    expect(r.out).toMatch(/not the size of any model Relay knows/);
    expect(r.out).toMatch(/ggml-base\.bin \(147951465\)/);
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('refuses on a checksum mismatch, and says why it matters', () => {
    // A church cannot check a checksum and will not suspect the file, so this stops
    // the build rather than warning about it.
    const src = read('scripts/offline-bundle.mjs');
    expect(src).toMatch(/did not match its checksum/);
    expect(src).toMatch(/whisper loads it and transcribes nonsense/);
    expect(src).toMatch(/A refusal, not a warning/);
  });

  it('needs both an installer and a model', () => {
    expect(run([]).code).toBe(2);
  });
});

describe('the Rust half verifies the same thing the download does', () => {
  const rs = read('src-tauri/src/models.rs');
  const fn = rs.slice(rs.indexOf('pub fn install_from_file'), rs.indexOf('/// A model file found'));

  it('matches by CONTENT, never by filename', () => {
    // A file called `ggml-base.bin` proves nothing.
    expect(fn).toMatch(/eq_ignore_ascii_case\(m\.sha256\)/);
    expect(fn).not.toMatch(/file_name\(\)\s*==/);
  });

  it('re-hashes at the destination, because the copy is what gets loaded', () => {
    // A failing USB stick can produce a good read followed by a bad one.
    expect(fn.match(/sha256_file/g)?.length).toBeGreaterThanOrEqual(2);
    expect(fn).toMatch(/\.part/);
    expect(fn).toMatch(/std::fs::rename/);
  });

  it('is held back during a service like every other model change', () => {
    // Copying 148 MB and reloading whisper is exactly as disruptive from a USB stick
    // as from the internet.
    expect(read('src-tauri/src/servicelock.rs')).toMatch(/"install_model_file"/);
  });

  it('the scan looks in three folders and does not recurse', () => {
    const scan = rs.slice(rs.indexOf('pub fn scan_for_models'));
    expect(scan).toMatch(/downloads_dir\(\)/);
    expect(scan).toMatch(/app_data_dir\(\)/);
    expect(scan).toMatch(/model_install_dir\(\)/);
    expect(scan.slice(0, 2000)).not.toMatch(/walkdir|read_dir\([\s\S]{0,80}recurs/i);
    // Size is a pre-filter, not a verdict.
    expect(scan).toMatch(/m\.bytes == meta\.len\(\)/);
  });
});

describe('language packs were NOT built, and the reason is recorded', () => {
  it('nothing can override the alias table from a file', () => {
    // An unsigned pack that can rewrite book names is a wrong-scripture-on-a-wall
    // vector, and the word doing the work in "signed language packs" is SIGNED.
    const rs = read('src-tauri/src/detection.rs');
    expect(rs).toMatch(/include_str!\("\.\.\/data\/book_aliases\.json"\)/);
    // No command anywhere accepts an alias file.
    const main = read('src-tauri/src/main.rs');
    expect(main).not.toMatch(/import_language|install_language_pack|load_aliases/);
  });

  it('the decision says what it would need first', () => {
    const d = read('docs/DECISIONS.md');
    const s = d.slice(d.indexOf('Signed language packs are NOT shipped'));
    expect(s).toMatch(/wrong-scripture-on-a-wall/);
    expect(s).toMatch(/native speaker who has actually reviewed the tables/);
    // …and why "the operator chose the file" is not equivalent to signing.
    expect(s).toMatch(/A malicious template can be ugly or blank/);
  });
});
