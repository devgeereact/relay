// ROOMS — a church's setup for one space, captured and re-applied.
//
// Single responsibility: decide WHAT is worth remembering about a room, and apply
// it back one piece at a time, reporting exactly which pieces did not take.
//
// ── Why applying is a list, not a call ────────────────────────────────────────
//
// Every setting here already has a command with its own contract and its own idea
// of what failure means: `set_stt_language`, `set_channel_display`,
// `select_voice_profile`, `set_setting`. Re-implementing them in Rust as one
// "apply a room" command would be a second implementation of each, and the two
// would drift about what is legal.
//
// So the console drives them one at a time — and the point of that is the failure
// case. A room applied to a machine where the projector has been unplugged and the
// USB microphone has moved to another port will apply four of six things. Reporting
// that as success is the panic-control lie in a smaller costume; reporting it as
// failure would be worse, because five-sixths of the operator's setup DID come back
// and they need to know which one to fix.
//
// ── What is NOT here, and why ─────────────────────────────────────────────────
//
// The audio thresholds. DECISIONS §19 and CLAUDE.md rule 12: nothing may compare a
// signal to a stored level. A noise floor captured in this hall three weeks ago,
// applied today with the heating on and forty more people in it, is exactly that
// assumption — and the failure it produced was Relay going **deaf to a quiet
// preacher, silently**. What the room observed is written down for a person to
// read; nothing reads it back.

/**
 * A snapshot of the current setup, ready to store.
 *
 * Only facts that are actually known are included. A field left out is a field the
 * room will not touch when it is applied — which is the right behaviour for
 * something captured on a machine where, say, no voice profile had been chosen.
 */
export function captureRoom({ inputDevice, language, targetMinutes, voiceProfileId, channels } = {}) {
  const s = {};
  // '' is a REAL value here: it means "the system default input", which is a
  // different thing from "this room does not remember a microphone".
  if (inputDevice !== undefined && inputDevice !== null) s.inputDevice = inputDevice;
  if (language !== undefined && language !== null) s.language = language;
  if (Number.isFinite(targetMinutes) && targetMinutes > 0) s.targetMinutes = targetMinutes;
  if (Number.isFinite(voiceProfileId)) s.voiceProfileId = voiceProfileId;
  // Screens are remembered by NAME, not by id. Ids are per-database; a name is what
  // an operator recognises, and it survives a screen being deleted and re-added —
  // which is exactly what happens when somebody re-cables a room.
  const displays = (channels ?? [])
    .filter((c) => c.render_target === 'native_window' && c.display !== undefined)
    .map((c) => ({ name: c.name, display: c.display ?? null }));
  if (displays.length) s.displays = displays;
  return s;
}

/**
 * What Relay observed about this room, in a sentence a person can read.
 *
 * Deliberately prose, and deliberately not machine-readable: the moment this became
 * a number in a column, something would read it back into the gate.
 */
export function observedNote(quality) {
  if (!quality) return '';
  const bits = [];
  if (typeof quality.snr_db === 'number') bits.push(`speech about ${Math.round(quality.snr_db)} dB above the room`);
  if (quality.denoise === false) bits.push('the microphone would not run at 48 kHz, so noise reduction was off');
  if (typeof quality.clip_ratio === 'number' && quality.clip_ratio > 0.02) bits.push('the input was clipping');
  return bits.length ? `Last time: ${bits.join('; ')}.` : '';
}

/**
 * Apply a room, one piece at a time.
 *
 * `deps` are the store wrappers, injected so this is testable without a backend.
 * Returns `{ applied, failed }` — `failed` carries a sentence per piece that did
 * not take, naming the piece. Never throws: a room that half-applies must report
 * that, not disappear into a caller's `catch`.
 */
export async function applyRoom(settings, deps = {}) {
  const applied = [];
  const failed = [];
  const s = settings ?? {};

  const step = async (key, label, run) => {
    if (!(key in s)) return; // not remembered — leave whatever is set now alone
    try {
      await run(s[key]);
      applied.push(label);
    } catch (e) {
      failed.push(`${label} — ${deps.humanError ? deps.humanError(e) : String(e?.message ?? e)}`);
    }
  };

  await step('inputDevice', 'microphone', (v) => deps.setInputDevice?.(v));
  await step('language', 'recognition language', (v) => deps.setSttLanguage?.(v));
  await step('targetMinutes', 'service length', (v) => deps.setServiceTarget?.(v));
  await step('voiceProfileId', 'voice profile', (v) => deps.selectVoiceProfile?.(v));

  if (Array.isArray(s.displays) && deps.setChannelDisplay) {
    const byName = new Map((deps.channels ?? []).map((c) => [c.name, c]));
    for (const d of s.displays) {
      const ch = byName.get(d.name);
      if (!ch) {
        // Named honestly. "A screen called X is not set up on this machine" is
        // something the operator can act on; silently skipping it is not.
        failed.push(`screen “${d.name}” — no screen by that name is set up here`);
        continue;
      }
      try {
        await deps.setChannelDisplay(ch.id, d.display);
        applied.push(`screen “${d.name}”`);
      } catch (e) {
        failed.push(
          `screen “${d.name}” — ${deps.humanError ? deps.humanError(e) : String(e?.message ?? e)}`,
        );
      }
    }
  }

  return { applied, failed };
}

/** One sentence about what happened, for the operator. */
export function describeApply({ applied, failed }, roomName) {
  if (!failed.length && applied.length) return `${roomName} is set up — ${applied.join(', ')}.`;
  if (!applied.length && failed.length)
    return `Nothing from ${roomName} could be applied. ${failed.join('. ')}.`;
  if (failed.length)
    // Both halves, always. Five-sixths of an operator's setup coming back is good
    // news they need, and the missing sixth is the thing they have to go and fix.
    return `${roomName}: ${applied.join(', ')} restored. Could not: ${failed.join('. ')}.`;
  return `${roomName} had nothing saved to apply.`;
}
