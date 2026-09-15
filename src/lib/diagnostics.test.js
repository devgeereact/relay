// RG-12 — a diagnostic file, not a diagnostic screen.
//
// Settings → Diagnostics has shown the right facts for a while and been useless for
// the job it exists for: nobody can email a screen. What actually happens is
// somebody photographs it with a phone, losing half the table and all of the
// latency history.
//
// The file is the one artefact in Relay that is EXPECTED to leave the building, so
// the tests that matter are about what is not in it.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const store = await import('./stores/capture.js');
const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');
const rs = read('src-tauri/src/main.rs');
const mod = read('src-tauri/src/diagnostics.rs');
const settings = read('src/lib/views/Settings.svelte');

/**
 * The body of `export_diagnostics`, from its signature to its closing brace.
 *
 * Every assertion about what the bundle may contain has to be scoped to the
 * command itself, and it has to be scoped by the code's own shape rather than by
 * a character count: a fixed window silently stops covering the end of the
 * function as soon as anything is added near the top, and it is the end that a
 * new field gets appended to.
 */
function exportDiagnostics() {
  const start = rs.indexOf('fn export_diagnostics(');
  expect(start).toBeGreaterThan(-1);
  const end = rs.indexOf('\n}\n', start);
  expect(end).toBeGreaterThan(start);
  return rs.slice(start, end);
}

beforeEach(() => {
  invoke.mockReset();
  store.capture.update((s) => ({ ...s, available: true }));
});

describe('the export', () => {
  it('returns where the file landed', async () => {
    invoke.mockResolvedValue('/Users/ada/Downloads/relay-diagnostics-1.md');
    await expect(store.exportDiagnostics()).resolves.toMatch(/relay-diagnostics/);
  });

  it('THROWS when it failed — an operator must not hunt for a file that was never written', async () => {
    invoke.mockRejectedValue(new Error('the disk is full'));
    await expect(store.exportDiagnostics()).rejects.toThrow();
  });

  it('the button says where it went, and what is in it', () => {
    // "Saved" with no path sends somebody hunting through a Downloads folder.
    expect(settings).toMatch(/Saved to \$\{path\}/);
    expect(settings).toMatch(/no transcript, verse text, lyric or service name/);
    expect(settings).toMatch(/you can read it before you send it/);
  });
});

describe('it is composed as an ALLOW-LIST', () => {
  it('says so, and says why', () => {
    // telemetry.rs promised an allow-list in its comment and shipped a blocklist
    // underneath that carried every field nobody had thought of. A blocklist here
    // would leak whatever the next feature adds.
    expect(mod).toMatch(/ALLOW-LIST/);
    expect(mod).toMatch(/blocklist here would leak\s+.{0,4}whatever the next feature adds/is);
  });

  it('the command reads no table that holds the church’s material', () => {
    const fn = exportDiagnostics();
    for (const forbidden of [
      'service_transcripts',
      'service_detections',
      'list_services',
      'list_songs',
      'list_announcements',
      'list_saved_scripture',
      'list_media',
      'list_plans',
    ]) {
      expect(fn).not.toContain(forbidden);
    }
  });

  it('sends the model’s FILENAME, never its path', () => {
    // The path is inside a home folder and names a person.
    //
    // Scoped to the FUNCTION, not to its first 6000 characters. The window was a
    // magic number and it expired the first time a fact was added above this one
    // (RG-122's microphone line): the guarantee still held, and the test failed
    // anyway. A scanner with an arbitrary edge reports on where code sits rather
    // than on what it does — the same defect `ipc.test.js` has had twice, in the
    // safer direction.
    const fn = exportDiagnostics();
    expect(fn).toMatch(/\.file_name\(\)/);
  });

  it('does not include the update snapshot path, only the version', () => {
    const fn = exportDiagnostics();
    const pending = fn.slice(fn.indexOf('Pending update'), fn.indexOf('Pending update') + 400);
    expect(pending).toMatch(/from_version/);
    expect(pending).not.toMatch(/\.snapshot/);
  });
});

describe('the home directory is scrubbed', () => {
  it('is applied to the whole document, not per field', () => {
    // A per-field version is one forgotten call away from a leak.
    expect(mod).toMatch(/scrub_paths\(body, home\.as_deref\(\)\)/);
    expect(mod).toMatch(/Applied to the WHOLE document at the end, not per field/);
  });

  it('is proven by a test on the Rust side, in both separators', () => {
    expect(mod).toMatch(/fn the_home_directory_is_replaced_everywhere_it_appears/);
    expect(mod).toMatch(/fn a_windows_home_is_scrubbed_in_either_separator/);
    // …and a degenerate HOME must not turn every slash into a tilde.
    expect(mod).toMatch(/fn a_useless_home_value_changes_nothing/);
  });
});

describe('the numbers in it obey the same rule as everywhere else', () => {
  it('a stage never reached prints a dash, never 0ms', () => {
    // The last hop of the rule latency.rs enforces in its histogram and
    // perf_samples enforces in the schema. "0ms" would tell whoever reads this file
    // that the fastest part of the pipeline was the part that never ran.
    expect(rs).toMatch(/fn an_unreached_stage_prints_a_dash_not_a_zero/);
    const ms = rs.slice(rs.indexOf('/// Milliseconds, or an em dash.'));
    expect(ms.slice(0, 300)).toMatch(/unwrap_or_else\(\|\| "—"/);
  });

  it('a metric with no samples is skipped rather than printed as zeros', () => {
    // THE WHOLE FUNCTION, not its first 8000 characters. The original window was
    // an arbitrary cap that happened to reach far enough when it was written; the
    // diagnostics bundle has gained facts since and the guard now sits past it, so
    // the test began failing over code that is present and correct. A scan whose
    // reach depends on how long the function happens to be is one that goes quiet
    // exactly as the function gets big enough to need it. `exportDiagnostics()`
    // slices to the end of the function and is already used by every other case
    // in this file.
    expect(exportDiagnostics()).toMatch(/if m\.samples == 0 \{\s*\n\s*continue;/);
  });
});
