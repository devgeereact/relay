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
    const fn = rs.slice(rs.indexOf('fn export_diagnostics('), rs.indexOf('#[cfg(test)]\nmod diagnostic_bundle_tests'));
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
    const fn = rs.slice(rs.indexOf('fn export_diagnostics('));
    expect(fn.slice(0, 6000)).toMatch(/\.file_name\(\)/);
  });

  it('does not include the update snapshot path, only the version', () => {
    const fn = rs.slice(rs.indexOf('fn export_diagnostics('));
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
    const fn = rs.slice(rs.indexOf('fn export_diagnostics('));
    expect(fn.slice(0, 8000)).toMatch(/if m\.samples == 0 \{\s*\n\s*continue;/);
  });
});
