// RG-138 — the recognition language must survive the launch it was chosen in.
//
// `Settings → Scripture & Languages → Recognition language` set a field on the live
// whisper engine and nothing else. `set_stt_language` took no `Db` at all, so an
// operator who chose English lost it at the next launch — and it LOOKED sticky for
// the rest of the run, because `stt_status` read the same live engine back.
//
// That is not a cosmetic gap. `docs/qa/RELAY_GAP.md` RG-116 names this control as
// the mitigation for a real field failure: whisper's language election wandered off
// English on `ggml-small` and produced 17 incoherent transcripts against 161 with
// the language pinned. The register named a fix that did not persist.
//
// The fix writes to the ACTIVE VOICE PROFILE — `voice_profiles.language`, which is
// already applied at startup, on a profile switch and after a model reload. One
// fact, one store. The two things this file holds are the frontend half:
//
//   1. the wrapper reaches the command and keeps the store in step, and
//   2. it THROWS when the write did not happen (it moved from GROUP 2 to GROUP 1),
//      because a swallowed failure leaves the select showing a language nothing was
//      told about — and `rooms.js` reports a room's steps by catching them.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { setSttLanguage, capture } = await import('./stores/capture.js');

describe('pinning the recognition language', () => {
  beforeEach(() => {
    invoke.mockReset();
    capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, language: null } }));
  });

  it('sends the code to the backend and returns the profile it was stored on', async () => {
    invoke.mockResolvedValue({ id: 1, name: 'Default', language: 'en', is_active: true });
    const profile = await setSttLanguage('en');
    expect(invoke).toHaveBeenCalledWith('set_stt_language', { language: 'en' });
    // The profile comes back so the surface can name whose calibration it changed.
    expect(profile.name).toBe('Default');
    expect(get(capture).stt.language).toBe('en');
  });

  it('sends null for auto-detect — an absent choice is a choice', async () => {
    invoke.mockResolvedValue({ id: 1, name: 'Default', language: null, is_active: true });
    await setSttLanguage(undefined);
    expect(invoke).toHaveBeenCalledWith('set_stt_language', { language: null });
  });

  it('THROWS when the language could not be stored — the caller must be able to say so', async () => {
    invoke.mockRejectedValue('the database is locked');
    await expect(setSttLanguage('yo')).rejects.toBeTruthy();
  });

  it('leaves the store on the OLD language when the write failed', async () => {
    // The half that matters, and the half an optimistic update would get wrong: if
    // nothing was stored, the select must not sit there reading "Yoruba". The
    // operator can choose again; a console that has already moved cannot tell them
    // there is anything to choose again.
    invoke.mockRejectedValue('the database is locked');
    await setSttLanguage('yo').catch(() => {});
    expect(get(capture).stt.language).toBe(null);
  });
});
