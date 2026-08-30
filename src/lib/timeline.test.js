// RG-04 — the service record, on the frontend side.
//
// Rust owns the merge and proves it (`db::services::timeline_tests`, plus the e2e
// test that runs a real service and reads its record back). This file covers the
// two things only the console can get wrong: pretending a service that recorded
// nothing recorded something, and printing a stage that was never reached as if it
// had been instantaneous.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import fs from 'node:fs';
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

describe('reading a service record', () => {
  it('returns what Rust merged, with each row still naming its source', async () => {
    invoke.mockResolvedValue([
      { at_ms: 0, source: 'event', kind: 'service_started', detail: 'Sunday' },
      { at_ms: 30500, source: 'detection', kind: 'auto', detail: 'John 3:16' },
      { at_ms: 40000, source: 'cue', kind: 'clear_screens', detail: null },
    ]);
    const rows = await store.serviceTimeline(1);
    expect(rows.map((r) => r.source)).toEqual(['event', 'detection', 'cue']);
  });

  it('degrades to an empty record rather than taking the Library down', async () => {
    // History is read-only. A failed query must not stop an operator reading the
    // transcript of the service they came here for.
    invoke.mockRejectedValue(new Error('database is locked'));
    await expect(store.serviceTimeline(1)).resolves.toEqual([]);
    await expect(store.servicePerf(1)).resolves.toEqual([]);
  });
});

describe('what the History screen may claim', () => {
  const view = read('src/lib/views/library/History.svelte');

  it('says "nothing was watching" for a service with no record, not "nothing happened"', () => {
    // Services recorded before this existed have no timeline. Telling somebody
    // their Sunday was uneventful, when in fact nothing was recording, is the same
    // wrong as the empty-vs-error bug this screen already fixed once.
    expect(view).toMatch(/nothing was watching/i);
  });

  it('prints an unreached stage as an absence, never as zero', () => {
    // The rule `latency.rs` enforces in memory and `perf_samples` enforces in the
    // schema has to survive the last hop too: `Math.round(null)` is 0, and a table
    // of zeroes reads as the fastest service ever recorded.
    expect(view).toMatch(/p50_ms === null \? '—'/);
    expect(view).toMatch(/p95_ms === null \? '—'/);
    expect(view).toMatch(/worst_ms === null \? '—'/);
  });

  it('a panic control that failed reads as a fault, and never in amber', () => {
    // Amber means ON AIR (DECISIONS §22) and nothing on the history screen is on
    // air. A failure here is rose.
    expect(view).toMatch(/panic_failed/);
    expect(view).toMatch(/isFault/);
    const styles = view.slice(view.indexOf('<style>'));
    expect(styles).toMatch(/\.lib-tl-row\.fault[\s\S]*?--v-rose/);
    expect(styles).not.toMatch(/\.lib-tl-row\.fault[\s\S]{0,200}--v-amber/);
  });

  it('keeps the three sources distinguishable on screen', () => {
    expect(view).toMatch(/SOURCE_WORD/);
    for (const src of ['event', 'cue', 'detection']) expect(view).toContain(`${src}:`);
  });
});

describe('the timeline carries no content', () => {
  it('the schema and the reader keep verse text and transcripts out', () => {
    // The one part of the history most likely to be sent to somebody. Pinned on
    // both sides so a future column cannot quietly widen it.
    const rs = read('src-tauri/src/db/services.rs');
    const merge = rs.slice(rs.indexOf('pub fn service_timeline'));
    const body = merge.slice(0, merge.indexOf('\n}\n'));
    expect(body).not.toMatch(/t\.text/);
    expect(body).not.toMatch(/heard_text/);
    expect(body).not.toMatch(/v\.text/);
  });
});
