// Crash guard for the operator console.
//
// There was no error boundary anywhere. An uncaught error in ANY view tore the
// whole console down to a white screen — mid-service, while the projector kept
// happily showing the last slide. The git history says this is a real wound and
// not a hypothetical one ("Fix silent crash after first transcript", "Fix the
// real freeze: tick() in a reactive block", "Fix freeze on Start listening").
// Svelte 4 has no <svelte:boundary> (that landed in Svelte 5), so this is the
// boundary.
//
// The single most important thing this file does is TELL THE TRUTH, calmly:
// the output windows are separate webviews with their own WebSocket reconnect
// loop, so when the console dies the congregation sees NOTHING happen. An
// operator who knows that does not panic. So we say it, in the largest words on
// the panel.
//
// Deliberately written in plain DOM with inline styles, not Svelte: the whole
// premise is that the Svelte app may be the thing that just broke, so the
// recovery UI must not depend on it — nor on the stylesheet having loaded.

// The ONE exception to "this file depends on nothing": the boot record, so the
// NEXT launch can show Crash Report Recovery instead of pretending the last run
// ended normally. `svelte/store` is a few hundred bytes of subscription
// bookkeeping, not the app — and the alternative, a second hard-coded copy of
// the localStorage key in this file, is how the two halves drift apart and the
// recovery screen silently stops appearing.
import { markCrash, markCleanExit } from './boot/boot.js';

const PANEL_ID = 'relay-crash-panel';

/** Best-effort read of where the operator was, for the "you'll resume at" line. */
function resumePoint() {
  try {
    const s = JSON.parse(localStorage.getItem('relay.session.v1') || '{}');
    const bits = [];
    if (s.activeTab) bits.push(`${s.activeTab} tab`);
    if (s.planId) bits.push(`plan #${s.planId}`);
    if (s.liveCueId) bits.push(`cue #${s.liveCueId}, slide ${(s.liveSlide ?? 0) + 1}`);
    return bits.length ? bits.join(' · ') : null;
  } catch {
    return null;
  }
}

function render(message) {
  // Record it BEFORE drawing anything. If the panel itself throws, or the user
  // yanks the power, the next boot must still know this run ended badly — that
  // is the whole input to Crash Report Recovery and to the safe-mode offer.
  try {
    markCrash(message);
  } catch {
    /* never let bookkeeping stop the recovery panel from appearing */
  }
  if (document.getElementById(PANEL_ID)) return; // already up — don't stack
  const resume = resumePoint();

  const el = document.createElement('div');
  el.id = PANEL_ID;
  el.setAttribute('role', 'alertdialog');
  el.setAttribute('aria-modal', 'true');
  el.setAttribute('aria-label', 'Console stopped responding');
  el.style.cssText = `
    position:fixed; inset:0; z-index:99999;
    display:flex; align-items:center; justify-content:center;
    background:#0a0a0a; color:#f2f2f2;
    font-family:Inter, system-ui, -apple-system, sans-serif;
    padding:24px; overflow:auto;
  `;

  const card = document.createElement('div');
  card.style.cssText = `
    max-width:620px; width:100%;
    background:#1b1b1b; border:1px solid rgba(255,255,255,.13);
    border-radius:14px; padding:32px;
  `;

  // Rose is used ONLY for the small error mark. The panel itself stays calm —
  // this screen exists to lower the operator's heart rate, not raise it.
  card.innerHTML = `
    <div style="display:flex;align-items:center;gap:10px;margin-bottom:20px;">
      <span style="width:9px;height:9px;border-radius:50%;background:#ef4444;flex:none;"></span>
      <span style="font-size:12px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:#ef4444;">
        Console error
      </span>
    </div>

    <h1 style="margin:0 0 14px;font-family:Inter,system-ui,sans-serif;font-size:26px;font-weight:600;line-height:1.25;">
      The console stopped responding.
    </h1>

    <p style="margin:0 0 22px;font-size:17px;line-height:1.55;color:#f2f2f2;">
      <strong style="color:#22c55e;">Your output screens are still live.</strong>
      The congregation sees no interruption — the projector, the stage monitor and
      any streaming feeds are separate windows and are still showing whatever you
      last put up.
    </p>

    <p style="margin:0 0 24px;font-size:14px;line-height:1.6;color:#b3b3b3;">
      Recovering reloads only this control panel. It will not blank the screens.
      ${resume ? `You'll come back to <span style="font-family:'JetBrains Mono',ui-monospace,monospace;color:#f2f2f2;">${resume}</span>.` : ''}
    </p>

    <div style="display:flex;gap:10px;flex-wrap:wrap;">
      <button id="relay-crash-recover" style="
        background:#ffb000;color:#241a00;border:0;border-radius:8px;
        padding:11px 20px;font-size:14px;font-weight:600;cursor:pointer;
        font-family:inherit;">
        Recover console
      </button>
      <button id="relay-crash-dismiss" style="
        background:transparent;color:#b3b3b3;border:1px solid rgba(255,255,255,.13);
        border-radius:8px;padding:11px 20px;font-size:14px;font-weight:500;
        cursor:pointer;font-family:inherit;">
        Dismiss and keep working
      </button>
    </div>

    <details style="margin-top:24px;">
      <summary style="cursor:pointer;font-size:12px;color:#8a8a8a;">
        Technical detail (for a bug report)
      </summary>
      <pre style="
        margin:10px 0 0;padding:12px;background:#141414;
        border:1px solid rgba(255,255,255,.075);border-radius:8px;
        font-family:'JetBrains Mono',ui-monospace,monospace;font-size:11px;
        line-height:1.5;color:#b3b3b3;white-space:pre-wrap;word-break:break-word;
        max-height:220px;overflow:auto;"></pre>
    </details>
  `;

  // textContent, not innerHTML — an error message can contain anything, and this
  // panel must never become an injection vector on its way to being helpful.
  card.querySelector('pre').textContent = message;

  el.appendChild(card);
  document.body.appendChild(el);

  const recover = card.querySelector('#relay-crash-recover');
  recover.addEventListener('click', () => window.location.reload());
  card.querySelector('#relay-crash-dismiss').addEventListener('click', () => el.remove());
  recover.focus();
}

function describe(err) {
  if (!err) return 'Unknown error';
  if (err instanceof Error) return `${err.name}: ${err.message}\n\n${err.stack ?? ''}`;
  try {
    return typeof err === 'string' ? err : JSON.stringify(err, null, 2);
  } catch {
    return String(err);
  }
}

export function installCrashGuard() {
  window.addEventListener('error', (e) => {
    // A failed resource load (missing image, font, video) also fires `error` on
    // window, but with the ELEMENT as the target. A missing asset is not a crash.
    //
    // Identify that by the target being an element, not by comparing it against
    // `window` — under a proxied global (jsdom, and some webview shims)
    // `e.target === window` is false even for a genuine script error, which
    // would have made this guard swallow every crash it exists to catch.
    if (e.target && typeof e.target.tagName === 'string') return;
    render(describe(e.error ?? e.message));
  });

  window.addEventListener('unhandledrejection', (e) => {
    render(describe(e.reason));
  });
}

/**
 * Warn before leaving/reloading while the mic is live. Wired from App.svelte,
 * which knows the capture state.
 */
export function installLeaveGuard(isCapturing) {
  const onBeforeUnload = (e) => {
    // Reaching `beforeunload` at all means the window is going away in an
    // orderly fashion, so the next boot has no crash to report. A hard kill or a
    // renderer death never gets here — which is exactly the distinction Crash
    // Report Recovery is trying to draw.
    try {
      markCleanExit();
    } catch {
      /* quota / private mode */
    }
    if (!isCapturing()) return;
    e.preventDefault();
    // Modern browsers ignore custom text, but returnValue must be set to prompt.
    e.returnValue = '';
    return '';
  };
  window.addEventListener('beforeunload', onBeforeUnload);
  return () => window.removeEventListener('beforeunload', onBeforeUnload);
}
