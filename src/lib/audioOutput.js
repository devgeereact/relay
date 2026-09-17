// The ONE place the operator's chosen speaker (audio output device) is read,
// written and applied to a media element.
//
// Why the browser and not cpal: the only way to route a <video>'s sound to a
// specific speaker is HTMLMediaElement.setSinkId(deviceId), and that deviceId
// comes from navigator.mediaDevices.enumerateDevices(). cpal device NAMES (what
// `list_audio_devices` returns for the mic) are a different namespace entirely —
// feeding one to setSinkId can never work. A cpal-backed speaker picker would
// look correct and route nothing, so this deliberately does not mirror the input
// path's backend enumeration.
//
// Storage is localStorage rather than the capture store because the fullscreen
// output runs in its OWN webview (channels.rs opens a separate window). Same
// origin means both windows share localStorage, so the operator's choice reaches
// the window that actually has the video — with no backend round-trip. The
// `storage` event syncs other windows; a CustomEvent covers the current one
// (`storage` does not fire in the window that wrote it).

export const AUDIO_OUTPUT_KEY = 'relay.audioOutput';
const CHANGE_EVENT = 'relay:audio-output';

/** Does this webview support choosing an output device at all? */
export function supportsSinkId() {
  return (
    typeof HTMLMediaElement !== 'undefined' &&
    typeof HTMLMediaElement.prototype?.setSinkId === 'function'
  );
}

/** Selected output deviceId, or '' for the system default. */
export function getAudioOutput() {
  try {
    return localStorage.getItem(AUDIO_OUTPUT_KEY) || '';
  } catch {
    return '';
  }
}

/** Persist the selection and notify this window + any output windows. */
export function setAudioOutput(deviceId) {
  const id = deviceId || '';
  try {
    localStorage.setItem(AUDIO_OUTPUT_KEY, id);
  } catch {
    // Private mode / storage disabled: the selection still applies to THIS
    // window via the event below, it just will not survive a reload.
  }
  try {
    window.dispatchEvent(new CustomEvent(CHANGE_EVENT, { detail: id }));
  } catch {
    /* no window (tests) */
  }
}

/** Subscribe to selection changes from this window or another. Returns unsub. */
export function onAudioOutputChange(fn) {
  const local = (e) => fn(e.detail ?? '');
  const cross = (e) => {
    if (e.key === AUDIO_OUTPUT_KEY) fn(e.newValue || '');
  };
  window.addEventListener(CHANGE_EVENT, local);
  window.addEventListener('storage', cross);
  return () => {
    window.removeEventListener(CHANGE_EVENT, local);
    window.removeEventListener('storage', cross);
  };
}

/**
 * Ask the webview for media permission so the device list becomes readable.
 *
 * Measured on this app's WKWebView (macOS): before any permission is granted,
 * enumerateDevices() returns placeholder entries with EMPTY deviceId/label and
 * NO audiooutput entries at all — which is why the speaker list reads as empty.
 * Granting once makes the real outputs (and their names) appear.
 *
 * The stream is stopped immediately: this only trips the permission, it never
 * holds the mic. That matters because the real capture path is cpal in Rust —
 * leaving a webview stream open would mean two things owning the microphone.
 *
 * ── WHY THIS RETURNS AN OBJECT AND NOT A BOOLEAN ────────────────────────────
 *
 * It used to return `false` for every unhappy path, so the one caller — Settings'
 * *Detect speakers* — re-rendered pixel for pixel in three different situations:
 * the operator declined the prompt, the machine has no input device to ask about,
 * and the permission was granted and this computer genuinely has one speaker. The
 * facts were here and were thrown away one line before the caller needed them:
 * `getUserMedia` rejects with a DOMException whose `name` already says which.
 *
 * Alternatives considered and rejected. A string enum (`'denied'` / `'granted'`)
 * reads well but every value is TRUTHY, so a caller writing the obvious
 * `if (await ensureDeviceAccess())` would treat a refusal as a success — silently,
 * which is the failure this function is being repaired for. THROWING on refusal
 * makes the commonest, most expected outcome an exception, and an outcome the UI
 * is designed to render is not exceptional. So: a plain object, `ok` for the one
 * boolean anybody needs and `reason` for the word the caller renders. The object
 * is truthy too, but nothing about it invites being read as a yes/no.
 *
 * @returns {Promise<{ ok: boolean, reason: 'granted'|'denied'|'no-input'|'unsupported'|'failed' }>}
 */
export async function ensureDeviceAccess() {
  if (!navigator?.mediaDevices?.getUserMedia) {
    return { ok: false, reason: 'unsupported' };
  }
  let stream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    return { ok: true, reason: 'granted' };
  } catch (e) {
    return { ok: false, reason: accessReason(e) };
  } finally {
    stream?.getTracks?.().forEach((t) => t.stop());
  }
}

/**
 * Which failure was it? Read from `DOMException.name`, the one thing about a
 * getUserMedia rejection that is specified rather than browser prose.
 *
 * `'failed'` is deliberately its own answer and not folded into `'denied'`: an
 * unrecognised name is a situation nobody has looked at, and telling an operator
 * to go and un-refuse a permission they never refused sends them to a checkbox
 * that is already ticked.
 */
function accessReason(e) {
  switch (e?.name) {
    case 'NotAllowedError':
    case 'PermissionDeniedError': // older spelling, still emitted by some webviews
    case 'SecurityError':
      return 'denied';
    case 'NotFoundError':
    case 'DevicesNotFoundError':
    case 'OverconstrainedError':
      return 'no-input';
    default:
      return 'failed';
  }
}

/**
 * Available speakers, shaped like the input list ({ id, label, is_default }).
 * May be EMPTY until ensureDeviceAccess() has succeeded — see above. The caller
 * surfaces that rather than showing a dead control.
 */
export async function listOutputDevices() {
  if (!navigator?.mediaDevices?.enumerateDevices) return [];
  let all;
  try {
    all = await navigator.mediaDevices.enumerateDevices();
  } catch {
    return [];
  }
  return all
    .filter((d) => d.kind === 'audiooutput')
    .map((d) => ({
      id: d.deviceId,
      label: d.label || '',
      is_default: d.deviceId === 'default',
    }));
}

/**
 * Route `el`'s sound to the selected device. Returns true only if the routing
 * actually took effect — a caller must never report success it did not achieve
 * (CLAUDE.md §15). Unsupported webview, revoked device, or a rejected promise
 * all report false, and playback continues on the system default.
 */
export async function applySink(el, deviceId) {
  if (!el || !supportsSinkId()) return false;
  try {
    await el.setSinkId(deviceId || '');
    return true;
  } catch {
    return false;
  }
}
