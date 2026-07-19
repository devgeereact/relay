// Turn a backend error into a sentence a church volunteer can act on.
//
// Relay's Tauri commands return `Result<_, String>` — 88 of them — so what reaches
// the frontend is a raw Rust error string. `Channels.svelte` rendered five of them
// verbatim, in a MONOSPACE font, to a volunteer who has never seen a stack trace:
//
//     Err("failed to bind 0.0.0.0:8032: Address already in use (os error 48)")
//
// That tells them nothing they can do. The audio path has excellent plain-language
// copy; the rest of the app had none. This is the one place that gets fixed, so a
// sixth view does not invent a sixth style.
//
// The rule: say what happened, and say what to DO. If we genuinely do not recognise
// the error, show it — but as a plain sentence with a lead-in, never as a bare
// monospace dump that reads like the app has crashed.

/** Patterns we recognise, most specific first. */
const KNOWN = [
  {
    // The LAN servers (kiosk WS, media/output HTTP) could not take their port.
    match: /address already in use|eaddrinuse|failed to bind/i,
    say: () =>
      'Another program is already using that network port. This is usually a second copy of Relay — close it and try again.',
  },
  {
    match: /monitor|display/i,
    say: (s) =>
      /not found|no such|invalid|out of range/i.test(s)
        ? 'That screen is not connected any more. Plug it back in, or pick a different one.'
        : null,
  },
  {
    match: /permission denied|not permitted|access is denied/i,
    say: () =>
      "Relay was not allowed to do that. On macOS, check System Settings → Privacy & Security; on Windows, allow Relay through the firewall when asked.",
  },
  {
    match: /no such file|not found.*model|model.*not found/i,
    say: () => 'The speech model is not on this machine yet. Download it from Settings.',
  },
  {
    match: /database is locked|sqlite/i,
    say: () => 'Relay could not save that just now. Try once more — if it keeps happening, restart Relay.',
  },
  {
    // THE ENGINE IS NOT THERE. This is what the Tauri bridge throws when the page
    // is open without a backend behind it — a browser pointed at the dev server,
    // or a webview that came up before the Rust side did.
    //
    // It surfaced verbatim on the FIRST-RUN WIZARD, which is the worst possible
    // place for `Cannot read properties of undefined (reading 'invoke')`: the
    // reader is a volunteer, on their first thirty seconds in the product, and
    // the words tell them nothing about what to do.
    match: /reading 'invoke'|reading "invoke"|__TAURI__|is not a function.*invoke/i,
    say: () =>
      "Relay's engine is not running, so nothing on this screen can work yet. If you opened this in a web browser, use the Relay app instead — the browser page has no engine behind it.",
  },
];

/**
 * Is retrying this worth the operator's time?
 *
 * The ONE question a live operator actually has, and for months the answer was
 * unknowable: every command returned `Result<_, String>`, so "the database is busy"
 * and "the disk is full" arrived as indistinguishable sentences. `error.rs` now sends
 * `{ kind, message }`.
 */
export const isRetryable = (e) => e?.kind === 'busy';

/** A deliberate refusal — nothing is broken, and the operator can fix it. */
export const isRefusal = (e) => e?.kind === 'refused' || e?.kind === 'not_found';

/**
 * A plain-language sentence for an error from the backend.
 *
 * Always returns something showable. Never returns an empty string, and never
 * returns a bare Rust `Err(...)` — an operator mid-service needs an instruction,
 * not a diagnosis.
 *
 * Handles BOTH shapes: the typed `{ kind, message }` from a Tauri command, and a
 * plain string (thrown by the bridge itself when there is no backend, and by the
 * modules that still speak `Result<_, String>` internally).
 */
export function humanError(e) {
  // Typed. The backend has already classified it, and for two kinds it has also
  // already written the sentence — trust it rather than re-guessing from a regex.
  if (e && typeof e === 'object' && typeof e.kind === 'string') {
    if (e.kind === 'refused' || e.kind === 'not_found') return e.message;
    if (e.kind === 'busy') return e.message; // "…try that again in a moment."
    // 'io' and 'internal' are unclassified as far as the OPERATOR is concerned, so
    // they fall through to the pattern table below, which knows how to turn a few of
    // them ("address already in use") into something actionable.
    return humanError(e.message);
  }

  const raw = String(e ?? '')
    .replace(/^Error:\s*/i, '')
    .trim();
  if (!raw) return 'Something went wrong, and Relay could not say what.';

  for (const k of KNOWN) {
    if (k.match.test(raw)) {
      const said = k.say(raw);
      if (said) return said;
    }
  }

  // Unrecognised. Show it — hiding it would be worse — but frame it as a sentence
  // so it does not read like a crash, and so the operator knows it is safe to retry.
  return `That didn't work: ${raw}`;
}
