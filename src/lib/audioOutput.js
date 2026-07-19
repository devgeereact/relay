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
 */
export async function ensureDeviceAccess() {
  if (!navigator?.mediaDevices?.getUserMedia) return false;
  let stream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    return true;
  } catch {
    return false; // operator declined, or no input exists
  } finally {
    stream?.getTracks?.().forEach((t) => t.stop());
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
