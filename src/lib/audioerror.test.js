// A microphone that dies mid-service must stop capture, be written down, and be
// said — on whatever tab the operator happens to be on.
//
// RG-117. `audio.rs`'s stream error callback was one line:
//
//   let err_fn = |e| eprintln!("audio stream error: {e}");
//
// It set no stop flag, emitted no event, and reached no operator. The capture loop
// is `while !stop` around a 100 ms `recv_timeout`, and the stream object stays
// alive, so the channel never disconnects: it spun on `Timeout => continue` for
// ever. Three consequences, none of them visible.
//
// The transcript simply stopped, which looks exactly like a preacher who has gone
// quiet. `RELAY_RECORD_WAV` writes AFTER the loop exits, so the recording of the
// failure was never written — on 2026-09-06 the desk feed was unplugged and 26
// minutes sat buffered in RAM while the app looked alive. And CLAUDE.md rule 5
// said device errors come back via `audio://error`, which was true of START errors
// and false of runtime ones.
//
// The Rust half is pinned in `audio.rs`. This file pins the half a volunteer sees.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import fs from 'node:fs';
import path from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture, dismissAudioError } = await import('./stores/capture.js');

const read = (p) => fs.readFileSync(path.resolve(process.cwd(), p), 'utf8');

describe('a dead microphone reaches the operator', () => {
  beforeEach(() => {
    invoke.mockReset();
    capture.update((s) => ({ ...s, audioError: null }));
  });

  it('is announced by the SHELL, not by one tab', () => {
    // Live had the only surface, and a volunteer setting up a template or reading
    // Help would never see it. The shell renders on every tab, which is the same
    // reason the panic bar lives there.
    const app = read('src/App.svelte');
    expect(app).toMatch(/\$capture\.audioError/);
    expect(app).toMatch(/class="audiobar"/);
    expect(app).toMatch(/role="alert"/);
  });

  it('says the microphone stopped, and carries the reason the device gave', () => {
    const app = read('src/App.svelte');
    const bar = app.slice(app.indexOf('class="audiobar"'), app.indexOf('class="audiobar"') + 400);
    expect(bar).toMatch(/stopped hearing the microphone/i);
    expect(bar).toMatch(/\{\$capture\.audioError\}/);
  });

  it('is rose, never amber and never amethyst', () => {
    // Rule 18's colour semantics: amber means ON AIR and is never allowed to lie,
    // amethyst means rehearsal. A fault is rose.
    const css = read('src/app.css');
    const rule = css.slice(css.indexOf('.audiobar{'), css.indexOf('.audiobar .panic-t'));
    expect(rule).toMatch(/--v-rose/);
    expect(rule).not.toMatch(/--v-amber|--v-amethyst/);
  });

  it('dismissing acknowledges the message and does not claim a repair', () => {
    capture.update((s) => ({ ...s, audioError: 'The requested device is no longer available.', capturing: false }));
    dismissAudioError();
    expect(get(capture).audioError).toBe(null);
    // Dismissing must not restart anything or claim the microphone is back.
    expect(get(capture).capturing).toBe(false);
    expect(invoke).not.toHaveBeenCalled();
  });

  it('the listener stops the console claiming the microphone is still live', () => {
    // `capturing: false` on the same update as the error. Without it the topbar
    // goes on showing a live microphone over a device that is gone.
    const store = read('src/lib/stores/capture.js');
    const at = store.indexOf("listen('audio://error'");
    expect(at).toBeGreaterThan(-1);
    expect(store.slice(at, at + 200)).toMatch(/capturing: false/);
  });
});

describe('the Rust half is pinned where it runs', () => {
  it('the stream error callback stops the loop instead of only printing', () => {
    const rs = read('src-tauri/src/audio.rs');
    // The exact shape of the bug: a callback whose whole body is a print.
    expect(rs).not.toMatch(/let err_fn = \|e\| eprintln!/);
    expect(rs).toMatch(/fn note_stream_error/);
    expect(rs).toMatch(/fn a_stream_error_stops_capture_and_keeps_its_reason/);
  });

  it('the recording is written before the error is reported', () => {
    // Reporting first would be correct-looking and wrong: `on_error` is what the
    // operator sees, and the audio that proves what happened has to be on disk by
    // then. The order is asserted here because it is invisible at the call site.
    const rs = read('src-tauri/src/audio.rs');
    const body = rs.slice(rs.indexOf('    drop(stream);'));
    expect(body.indexOf('write_wav_f32')).toBeLessThan(body.indexOf('return Err(msg)'));
  });
});
