// RG-10 — a room, remembered.
//
// The two things this could get wrong, and only one of them is about convenience:
//
//   1. Reporting a room as applied when half of it did not take. A room applied on
//      a machine where the projector moved and the USB microphone changed port will
//      restore four of six things — and BOTH halves of that are news the operator
//      needs.
//   2. Remembering an audio level and putting it back. DECISIONS §19 / rule 12:
//      nothing may compare a signal to a stored level, and the failure that rule
//      came from was Relay going deaf to a quiet preacher, silently.
import { describe, it, expect, vi } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { captureRoom, observedNote, applyRoom, describeApply } from './rooms.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

const CHANNELS = [
  { id: 1, name: 'Main screen', render_target: 'native_window', display: '1' },
  { id: 2, name: 'Stage', render_target: 'native_window', display: null },
  { id: 3, name: 'OBS', render_target: 'network_client', display: undefined },
];

describe('what a room remembers', () => {
  it('captures the choices that currently have to be re-made every week', () => {
    const s = captureRoom({
      inputDevice: 'Shure SM58',
      language: 'yo',
      targetMinutes: 90,
      voiceProfileId: 4,
      channels: CHANNELS,
    });
    expect(s).toEqual({
      inputDevice: 'Shure SM58',
      language: 'yo',
      targetMinutes: 90,
      voiceProfileId: 4,
      displays: [
        { name: 'Main screen', display: '1' },
        { name: 'Stage', display: null },
      ],
    });
  });

  it('treats "" as a real microphone choice — it means the system default', () => {
    // Dropping it would make "this room uses whatever is default" indistinguishable
    // from "this room does not remember a microphone", and the two behave differently
    // when the room is applied.
    expect(captureRoom({ inputDevice: '' })).toEqual({ inputDevice: '' });
    expect(captureRoom({})).toEqual({});
  });

  it('remembers screens by NAME, not by id', () => {
    // Ids are per-database. A name is what an operator recognises, and it survives a
    // screen being deleted and re-added — which is exactly what happens when
    // somebody re-cables a room.
    const s = captureRoom({ channels: CHANNELS });
    expect(s.displays.every((d) => 'name' in d && !('id' in d))).toBe(true);
  });

  it('leaves out what it does not know, so applying it touches nothing', () => {
    expect(captureRoom({ targetMinutes: 0, voiceProfileId: undefined })).toEqual({});
  });
});

describe('THE RULE — no audio level is ever stored to be applied', () => {
  it('captures no threshold, floor, gain or level of any kind', () => {
    const s = captureRoom({
      inputDevice: 'x',
      language: 'en',
      targetMinutes: 60,
      voiceProfileId: 1,
      channels: CHANNELS,
    });
    const keys = JSON.stringify(s).toLowerCase();
    for (const banned of ['noise', 'floor', 'threshold', 'gain', 'rms', 'vad', 'snr', 'level'])
      expect(keys).not.toContain(banned);
  });

  it('writes what it observed as PROSE, for a person', () => {
    // The moment this becomes a number in a field, something reads it back.
    const note = observedNote({ snr_db: 14.4, denoise: false, clip_ratio: 0.03 });
    expect(note).toMatch(/Last time:/);
    expect(note).toMatch(/14 dB above the room/);
    expect(note).toMatch(/48 kHz/);
    expect(note).toMatch(/clipping/);
    // …and it is a sentence, not a structure.
    expect(typeof note).toBe('string');
  });

  it('says nothing when nothing was observed', () => {
    expect(observedNote(null)).toBe('');
    expect(observedNote({})).toBe('');
  });

  it('the schema itself forbids a threshold column', () => {
    const rs = read('src-tauri/src/db/environments.rs');
    expect(rs).toMatch(/no_column_here_can_become_an_audio_threshold/);
    expect(rs).toMatch(/DECISIONS §19/);
  });
});

describe('applying a room, one piece at a time', () => {
  const deps = (over = {}) => ({
    setInputDevice: vi.fn(),
    setSttLanguage: vi.fn(),
    setServiceTarget: vi.fn(),
    selectVoiceProfile: vi.fn(),
    setChannelDisplay: vi.fn(),
    channels: CHANNELS,
    humanError: (e) => String(e?.message ?? e),
    ...over,
  });

  it('applies every remembered piece', async () => {
    const d = deps();
    const r = await applyRoom(
      { inputDevice: 'SM58', language: 'yo', targetMinutes: 90, voiceProfileId: 4, displays: [{ name: 'Main screen', display: '2' }] },
      d,
    );
    expect(d.setInputDevice).toHaveBeenCalledWith('SM58');
    expect(d.setSttLanguage).toHaveBeenCalledWith('yo');
    expect(d.setServiceTarget).toHaveBeenCalledWith(90);
    expect(d.selectVoiceProfile).toHaveBeenCalledWith(4);
    expect(d.setChannelDisplay).toHaveBeenCalledWith(1, '2');
    expect(r.failed).toEqual([]);
    expect(r.applied).toHaveLength(5);
  });

  it('selects the voice profile BEFORE it sets the language', async () => {
    // Not a tidiness assertion — the order is the whole fix.
    //
    // `set_stt_language` writes `voice_profiles.language` on whichever profile is
    // ACTIVE. With the language first, a room captured while "Guest" (en) was
    // active and applied on a Sunday when "Pastor Ade" (yo) is active overwrote
    // Pastor Ade's Yoruba pin with English, THEN switched to Guest — so the pin
    // was destroyed and the language never reached the profile the room named,
    // while `applied` reported "recognition language" as restored. Pinning the
    // language by hand is the RG-116 mitigation; a control that undoes it and says
    // it worked is rule 35.
    const d = deps();
    await applyRoom({ language: 'en', voiceProfileId: 7 }, d);
    expect(d.selectVoiceProfile).toHaveBeenCalledWith(7);
    expect(d.setSttLanguage).toHaveBeenCalledWith('en');
    expect(d.selectVoiceProfile.mock.invocationCallOrder[0]).toBeLessThan(
      d.setSttLanguage.mock.invocationCallOrder[0],
    );
  });

  it('does NOT write the language when the voice profile could not be selected', async () => {
    // The tail of the same defect. Ordering the two steps closes the case where
    // both work; it does nothing for the case where the profile step fails — the
    // language would then land on whichever profile HAPPENED to stay active, which
    // is the wrong preacher's row overwritten, on a Sunday. And a step that did not
    // run may not be reported as applied: the operator has to be told which of the
    // six things to go and fix.
    const d = deps({
      selectVoiceProfile: vi.fn(async () => {
        throw new Error('that profile has been deleted');
      }),
    });
    const r = await applyRoom({ language: 'en', voiceProfileId: 7, inputDevice: 'SM58' }, d);

    expect(d.setSttLanguage, 'the language was written to whatever stayed active').not.toHaveBeenCalled();
    expect(r.applied).toEqual(['microphone']);
    expect(r.applied).not.toContain('recognition language');
    expect(r.failed).toHaveLength(2);
    expect(r.failed[1]).toMatch(/recognition language — not attempted/);
    // …and the operator is told, in the one sentence they read.
    expect(describeApply(r, '“Main hall”')).toMatch(/recognition language — not attempted/);
  });

  it('still applies the language when the room remembers no profile at all', async () => {
    // The control. A guard that skipped the language whenever the profile step was
    // not `applied` would also skip it for every room captured before a profile was
    // ever chosen — most of them.
    const d = deps();
    await applyRoom({ language: 'yo' }, d);
    expect(d.setSttLanguage).toHaveBeenCalledWith('yo');
  });

  it('leaves a setting alone when the room does not remember it', async () => {
    const d = deps();
    await applyRoom({ language: 'en' }, d);
    expect(d.setSttLanguage).toHaveBeenCalled();
    expect(d.setInputDevice).not.toHaveBeenCalled();
    expect(d.setServiceTarget).not.toHaveBeenCalled();
  });

  it('NEVER THROWS — a room that half-applies must report that, not vanish', async () => {
    const d = deps({
      setInputDevice: vi.fn(async () => {
        throw new Error('that microphone is not plugged in');
      }),
    });
    const r = await applyRoom({ inputDevice: 'SM58', language: 'yo' }, d);
    expect(r.applied).toEqual(['recognition language']);
    expect(r.failed).toEqual(['microphone — that microphone is not plugged in']);
  });

  it('one failure does not stop the rest', async () => {
    const d = deps({
      setSttLanguage: vi.fn(async () => {
        throw new Error('nope');
      }),
    });
    const r = await applyRoom({ inputDevice: 'x', language: 'yo', targetMinutes: 60 }, d);
    expect(r.applied).toEqual(['microphone', 'service length']);
    expect(r.failed).toHaveLength(1);
  });

  it('names a screen this machine does not have, rather than skipping it', async () => {
    const d = deps();
    const r = await applyRoom({ displays: [{ name: 'Balcony', display: '3' }] }, d);
    expect(d.setChannelDisplay).not.toHaveBeenCalled();
    expect(r.failed[0]).toMatch(/Balcony.*no screen by that name is set up here/);
  });
});

describe('what the operator is told', () => {
  it('a clean apply names what came back', () => {
    expect(describeApply({ applied: ['microphone', 'service length'], failed: [] }, '“Main hall”'))
      .toBe('“Main hall” is set up — microphone, service length.');
  });

  it('a PARTIAL apply reports both halves', () => {
    // Five-sixths of an operator's setup coming back is good news they need; the
    // missing sixth is the thing they have to go and fix. Reporting only one half
    // is the panic-control lie in a smaller costume.
    const msg = describeApply(
      { applied: ['microphone'], failed: ['screen “Balcony” — no screen by that name'] },
      '“Main hall”',
    );
    expect(msg).toMatch(/microphone restored/);
    expect(msg).toMatch(/Could not: screen “Balcony”/);
  });

  it('a total failure never reads as a success', () => {
    const msg = describeApply({ applied: [], failed: ['microphone — gone'] }, '“Youth room”');
    expect(msg).toMatch(/^Nothing from “Youth room” could be applied/);
  });

  it('an empty room says so rather than claiming to have done something', () => {
    expect(describeApply({ applied: [], failed: [] }, '“New”')).toMatch(/nothing saved to apply/);
  });
});

describe('where it lives', () => {
  const settings = read('src/lib/views/Settings.svelte');

  it('sits with the microphone and the speakers, where a room is configured', () => {
    // It is a GROUP inside the section that describes the ROOM, headed by the
    // frame's `.rw-group` (docs/REBRAND.md §11's one type scale). That section was
    // called Audio and is called This room: a saved room puts back the microphone,
    // the recognition language, the planned length and which display each screen
    // goes to, so naming it for one of the five was always the narrower word.
    // What is asserted is unchanged: the rooms panel is in Settings, beside the
    // microphone that is one of the things a room remembers.
    expect(settings).toMatch(/<div class="rw-group">Rooms<\/div>/);
    const room = settings.slice(
      settings.indexOf("section === 'room'"),
      settings.indexOf("section === 'preachers'"),
    );
    expect(room).toMatch(/<div class="rw-group">Rooms<\/div>/);
    expect(room).toMatch(/<div class="rw-group">Microphone<\/div>/);
    expect(room).toMatch(/doSaveRoom/);
    expect(room).toMatch(/doUseRoom/);
  });

  it('tells the operator, in the UI, that levels are NOT saved and why', () => {
    // The most likely misunderstanding this feature can create, answered where it
    // will be had rather than in a decision log nobody in a booth will read.
    expect(settings).toMatch(/The audio levels are not saved/);
    expect(settings).toMatch(/deaf to a quiet preacher/);
  });
});
