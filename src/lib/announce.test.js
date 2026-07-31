// The wrappers wired for the emergency announcement, the repeat badge, and voice
// profiles — pinned to the THROW-vs-SWALLOW contract at the top of capture.js.
//
// That contract is the one thing this file exists to defend. All three of these
// were dead-but-built commands: registered in Rust, tested in Rust, and called by
// nothing. Wiring them meant choosing a group for each, and "it seemed fine" is
// exactly how a panic key came to do nothing.
//
// The choices, and why:
//
//   pushAnnouncement  — THROWS. It paints over live scripture on every screen at
//     once, for a fire alarm or a blocked car park. If it fails silently the
//     operator believes the room has been warned when it has not been.
//
//   verseRepeatCount  — SWALLOWS, returns 0. A "shown earlier" badge is an
//     affordance. If it fails the operator sees what they saw before the badge
//     existed. Nothing on any screen changes, so nothing is hidden from them.
//
//   voice profile WRITES — THROW. Selecting or saving a profile changes the STT
//     language and the gate thresholds: what the AI may put on a screen without
//     asking. A selection that failed silently leaves the desk calibrated for the
//     wrong preacher with nothing on screen to say so.
//
//   voice profile READS — SWALLOW. An empty list is visibly empty.
import { describe, it, expect, beforeEach, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const {
  pushAnnouncement,
  verseRepeatCount,
  listVoiceProfiles,
  activeVoiceProfile,
  createVoiceProfile,
  updateVoiceProfile,
  selectVoiceProfile,
  deleteVoiceProfile,
} = await import('./stores/capture.js');

// Block body — see nav.test.js. A concise arrow returns the mock, and vitest
// treats a value returned from beforeEach as a teardown function.
beforeEach(() => {
  invoke.mockReset();
});

describe('the emergency announcement is never silent about failing', () => {
  it('reaches the backend with the message', async () => {
    invoke.mockResolvedValue(undefined);
    await pushAnnouncement('Fire alarm — please leave by the side doors');
    expect(invoke).toHaveBeenCalledWith('push_announcement', {
      message: 'Fire alarm — please leave by the side doors',
    });
  });

  // THE POINT OF THE WHOLE FILE. Swallow this and the operator is told the
  // congregation has been warned when it has not been.
  it('THROWS when the backend refuses — it must never look like it worked', async () => {
    invoke.mockRejectedValue(new Error('no output channels are open'));
    await expect(pushAnnouncement('Doctor needed at the back')).rejects.toThrow(
      'no output channels are open',
    );
  });
});

describe('the repeat badge costs nothing when it fails', () => {
  it('passes the reference through as the backend parses it', async () => {
    invoke.mockResolvedValue(2);
    expect(await verseRepeatCount('John 3:16')).toBe(2);
    expect(invoke).toHaveBeenCalledWith('verse_repeat_count', { reference: 'John 3:16' });
  });

  it('returns 0 rather than throwing — a missing badge is not a live failure', async () => {
    invoke.mockRejectedValue(new Error('db locked'));
    await expect(verseRepeatCount('John 3:16')).resolves.toBe(0);
  });

  // No service being recorded is the common case on a Tuesday, and the backend
  // answers 0 for it. Null must read the same rather than becoming NaN in the
  // `> 0` test that drives the badge.
  it('treats a null answer as no repeats', async () => {
    invoke.mockResolvedValue(null);
    await expect(verseRepeatCount('John 3:16')).resolves.toBe(0);
  });
});

describe('voice profiles: reads degrade, writes report', () => {
  it('lists profiles, and returns an empty list if it cannot', async () => {
    invoke.mockResolvedValue([{ id: 1, name: 'Pastor Ade' }]);
    expect(await listVoiceProfiles()).toHaveLength(1);
    invoke.mockRejectedValue(new Error('no backend'));
    await expect(listVoiceProfiles()).resolves.toEqual([]);
  });

  it('returns null for the active profile rather than throwing', async () => {
    invoke.mockRejectedValue(new Error('no backend'));
    await expect(activeVoiceProfile()).resolves.toBeNull();
  });

  it('every WRITE throws, because each one re-aims the gate', async () => {
    invoke.mockRejectedValue(new Error('profile is in use'));
    await expect(createVoiceProfile('Ade', null)).rejects.toThrow();
    await expect(updateVoiceProfile({ id: 1, name: 'Ade' })).rejects.toThrow();
    await expect(selectVoiceProfile(1)).rejects.toThrow();
    await expect(deleteVoiceProfile(1)).rejects.toThrow();
  });

  // The form edits a copy and sends the WHOLE profile back, including the learned
  // auto_fire/suggest it never lets the operator touch. The backend decides whether
  // to keep or re-derive them (`thresholds_on_profile_save`) by comparing the
  // sensitivity dial — so the wrapper must not drop or reshape the object.
  it('sends the profile through whole, so the backend can compare sensitivity', async () => {
    invoke.mockResolvedValue({ id: 7 });
    const profile = {
      id: 7,
      name: 'Pastor Ade',
      language: 'yo',
      sensitivity: 60,
      auto_fire: 0.71,
      suggest: 0.42,
      bias_terms: 'Ekiti, Oyelaran',
      is_active: true,
    };
    await updateVoiceProfile(profile);
    expect(invoke).toHaveBeenCalledWith('update_voice_profile', { profile });
  });
});
