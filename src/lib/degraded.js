// DEGRADED — the fallbacks that already existed, made visible.
//
// Single responsibility: given what Relay currently knows about itself, say — in
// one sentence per problem, in an operator's words — which of its capabilities are
// reduced right now, and what to do about each.
//
// ── Why this file exists ──────────────────────────────────────────────────────
//
// Relay degrades gracefully in half a dozen places, and every one of those
// fallbacks was invisible. The denoiser silently switches off on a microphone that
// will not run at 48 kHz. With no speech model the app runs audio-only, which is a
// perfectly good manual tool and looks identical to a broken one. A build without a
// GPU backend decodes several times slower. Detection can be disarmed by a keypress
// nobody remembers pressing. Safe mode disarms every output on purpose.
//
// In each case Relay knew, and the operator did not — so the symptom ("it isn't
// hearing anything") got attributed to the AI being bad, which is the most expensive
// possible misdiagnosis for this product.
//
// ── The two rules ─────────────────────────────────────────────────────────────
//
// 1. **Nothing here is invented.** A row appears only when something Relay actually
//    measured says so. There is no "probably" and no heuristic; if a fact is not
//    available, no row is produced. An advisory that is sometimes wrong is an
//    advisory an operator learns to scroll past.
// 2. **Every row says what it means and what to do.** "Degraded" on its own is a
//    mood, not information. `what` is the consequence for the service; `fix` is the
//    next action, or an honest "nothing to do here" when there genuinely is none.
//
// Pure, so the wording and the thresholds can be tested without a backend.

/**
 * Severity, and it is only ever these two.
 *
 * There is no "error" level: an actual failure is not a degradation — it is a
 * failure, and it has its own louder surfaces (the panic banner, `audio://error`,
 * a screen going rose). Everything here is Relay still working, less well.
 *
 * - `blocked`  — a whole capability is unavailable. Relay runs; that part does not.
 * - `reduced`  — it works, but worse than it should, in a way that will be noticed.
 */
export const LEVELS = ['blocked', 'reduced'];

/**
 * @param s.sttLoaded      is a speech model loaded?
 * @param s.detectionOn    is detection armed?
 * @param s.capturing      is the microphone live?
 * @param s.safeMode       is safe mode on?
 * @param s.denoise        is the denoiser running? (null = not capturing, so unknown)
 * @param s.gpuBackends    what whisper.cpp was COMPILED with — a build fact
 * @param s.macos          is this macOS? (where a CPU-only build is a known trap)
 * @param s.droppedPartials how many decode passes have been shed
 * @param s.screensDown    names of screens that are attached but not answering
 */
export function degradations(s = {}) {
  const out = [];

  // ── Blocked ───────────────────────────────────────────────────────────────

  if (s.safeMode) {
    out.push({
      id: 'safemode',
      level: 'blocked',
      title: 'Safe mode is on',
      what: 'Outputs will not open and detection is disarmed — nothing Relay does can reach a screen.',
      fix: 'Settings → Backup & Recovery → Turn off safe mode.',
    });
  }

  if (s.sttLoaded === false) {
    // NOT an error. This is a completely usable manual tool, and saying so is the
    // difference between an operator who carries on and one who assumes Relay is
    // broken and stops.
    out.push({
      id: 'stt',
      level: 'blocked',
      title: 'No speech model — Relay is not listening for verses',
      what: 'Nothing will be transcribed or detected. Firing verses by hand works exactly as normal.',
      fix: 'Settings → Network → download a speech model.',
    });
  }

  // Only worth saying while the microphone is actually live: detection being off
  // with nothing playing into it is not a degradation, it is Tuesday.
  if (s.detectionOn === false && s.capturing && s.sttLoaded !== false) {
    out.push({
      id: 'detection',
      level: 'blocked',
      title: 'Detection is off',
      what: 'Relay is transcribing but will not suggest or fire anything.',
      fix: 'Turn it back on from the Live tab, or press the detection toggle.',
    });
  }

  // ── Reduced ───────────────────────────────────────────────────────────────

  if (s.denoise === false) {
    out.push({
      id: 'denoise',
      level: 'reduced',
      title: 'Noise reduction is off',
      what: 'This microphone will not run at 48 kHz, so the denoiser cannot. Speech is still cleaned up by the gain stage, but a noisy room will be harder to transcribe.',
      fix: 'Try a different microphone or input device if the transcript struggles.',
    });
  }

  // A BUILD fact, not a hardware one. Naming the GPU in this machine next to a
  // CPU-only build would be the most convincing lie on the screen.
  if (s.macos && Array.isArray(s.gpuBackends) && s.gpuBackends.length === 0) {
    out.push({
      id: 'gpu',
      level: 'reduced',
      title: 'This build has no GPU acceleration',
      what: 'Speech decoding runs on the processor, which on this platform is roughly three times slower — the transcript will lag the preacher on anything but the smallest model.',
      fix: 'Nothing you can change here; this is how this copy of Relay was built.',
    });
  }

  if ((s.droppedPartials ?? 0) > 0) {
    out.push({
      id: 'shed',
      level: 'reduced',
      title: `${s.droppedPartials} transcript ${s.droppedPartials === 1 ? 'update' : 'updates'} skipped`,
      what: 'Relay fell behind and dropped some in-progress updates to catch up. Nothing final was lost, and no verse was missed because of it.',
      fix: 'If it keeps climbing, a smaller speech model will keep up better — Settings → Diagnostics shows the speed.',
    });
  }

  const down = s.screensDown ?? [];
  if (down.length) {
    out.push({
      id: 'screens',
      level: 'reduced',
      title: down.length === 1 ? `${down[0]} is not responding` : `${down.length} screens are not responding`,
      what: 'Relay is still sending to them. They have stopped reporting that they are showing anything.',
      fix: 'Check the screen, the cable, or the browser source. Outputs tab has the detail.',
    });
  }

  return out;
}

/** The single worst level present, or null when everything is fine. */
export function worstLevel(list = []) {
  if (list.some((d) => d.level === 'blocked')) return 'blocked';
  if (list.length) return 'reduced';
  return null;
}

/**
 * One sentence for the shell, when there is no room for the list.
 *
 * Names the count rather than the first item: "3 things reduced" is scannable, and
 * picking one to display would imply a ranking Relay has not earned.
 */
export function summarise(list = []) {
  if (!list.length) return '';
  const blocked = list.filter((d) => d.level === 'blocked');
  if (blocked.length === 1) return blocked[0].title;
  if (blocked.length > 1) return `${blocked.length} things are unavailable`;
  if (list.length === 1) return list[0].title;
  return `${list.length} things are working, but not fully`;
}
