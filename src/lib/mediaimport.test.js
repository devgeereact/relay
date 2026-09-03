// IMPORTING A FILE MUST NOT BE HOW RELAY DIES.
//
// An imported file does not arrive as a path. The webview's `<input type=file>`
// hands back bytes, so `fileToBase64` builds the whole file as a binary string,
// then as a base64 string, Tauri serialises that string across the IPC bridge, and
// Rust decodes a further complete copy before writing it to disk. Four copies of
// one file exist at the peak, and nothing anywhere checked the size.
//
// For the 1.5 GB service video a volunteer drags into the Library on a Saturday,
// on the church's own laptop, that is not a slow import. It is the operating
// system killing Relay: no error, no message, nothing in any log, and no idea what
// they did wrong.
//
// The guard lives in `fileToBase64` and not at the four call sites, for the same
// reason `broadcast_with_clock` holds the pre-air validator (CLAUDE.md rule 36):
// media, graphics, documents and lyric files all import through this one function,
// and a guard added per call site is the guard that will be missing from the fifth.
//
// The backend holds the same limit (`main::MAX_IMPORT_BYTES`, pinned by
// `import_guard_tests`) because a Tauri command is invokable from the webview
// whatever the UI does. This half is the one that prevents the allocation.
import { describe, it, expect, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { fileToBase64, MAX_IMPORT_BYTES } = await import('./stores/capture.js');

/** A stand-in for a picked file. `arrayBuffer` is only ever reached on the path
 *  the guard is supposed to let through — a test that calls it on the refused
 *  path is a test proving the allocation still happens. */
function pickedFile(name, size, bytes = null) {
  return {
    name,
    size,
    arrayBuffer: async () => {
      if (!bytes) throw new Error('arrayBuffer() must not be reached for a refused file');
      return bytes.buffer;
    },
  };
}

describe('importing a file', () => {
  it('reads an ordinary file', async () => {
    const bytes = new Uint8Array([104, 105]); // "hi"
    const got = await fileToBase64(pickedFile('logo.png', bytes.length, bytes));
    expect(got).toBe('aGk=');
  });

  it('refuses a file larger than the limit BEFORE reading a byte of it', async () => {
    const huge = pickedFile('service.mp4', MAX_IMPORT_BYTES + 1);
    await expect(fileToBase64(huge)).rejects.toThrow(/service\.mp4/);
  });

  it('refuses as a REFUSAL, so the operator reads the sentence and not a diagnosis', async () => {
    const huge = pickedFile('service.mp4', MAX_IMPORT_BYTES * 4);
    const err = await fileToBase64(huge).catch((e) => e);
    // The shape `humanError` prints verbatim: nothing is broken, and the operator
    // can act on it.
    expect(err.kind).toBe('refused');
    expect(err.message).toMatch(/256 MB/);
    expect(err.message).toMatch(/compress/i);
  });

  it('accepts a file exactly at the limit — the cap is a ceiling, not a fence', async () => {
    const bytes = new Uint8Array([0]);
    const at = pickedFile('big.mp4', MAX_IMPORT_BYTES, bytes);
    await expect(fileToBase64(at)).resolves.toBeTypeOf('string');
  });

  it('holds the same number the backend holds', () => {
    // `main::MAX_IMPORT_BYTES`. Two limits that disagree is one limit that is
    // wrong, and the disagreement would surface as a file the UI accepted and the
    // engine refused after the whole thing had already been read into memory.
    expect(MAX_IMPORT_BYTES).toBe(256 * 1024 * 1024);
  });
});
