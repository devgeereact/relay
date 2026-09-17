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
// Settings' mount awaits this one before it asks for anything; unmocked it never
// resolves under jsdom and the sequence under test is simply never reached.
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('0.0.0-test') }));

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

// ── THE OTHER HALF: THE EDITOR MUST NOT PUT THE OLD LANGUAGE BACK ────────────
//
// The sections of Settings are `{#if}` branches of ONE component, so `editing` —
// the working copy `openEditor` takes — survives a walk to another section and
// back. Open the profile editor to nudge sensitivity, walk to Scripture &
// Languages, pin Yoruba, come back and press Save profile, and
// `update_voice_profile` was sent the STALE `language`. `main.rs` writes it and
// calls `apply_profile`, so the database AND the live engine both reverted,
// silently — and nothing on that path touched `capture.stt.language`, so the
// select on the other tab went on displaying the Yoruba it had just been moved
// away from. Two surfaces disagreeing about one column is the defect RG-138 exists
// for, one tab along, and pinning the language by hand is RG-116's mitigation.
//
// DRIVEN, not scanned: this is a SEQUENCE, and a source assertion cannot hold a
// sequence. Watched to go red by removing the `editing = { ...editing, language }`
// line from `pickLanguage`.
describe('Settings → the profile editor follows a language pinned on another tab', () => {
  const ADE = {
    id: 1,
    name: 'Pastor Ade',
    language: 'en',
    sensitivity: 50,
    auto_fire: 0.5,
    suggest: 0.35,
    bias_terms: '',
    is_active: true,
  };

  let host;
  let app;

  /** MACROTASKS, not microtasks — `capture.js` reaches the bridge through a lazy
   *  `await import(...)`, which does not resolve on a microtask drain. Same shape
   *  as `readstates.test.js`, which drives this same view. */
  const settle = async (n = 14) => {
    const { tick } = await import('svelte');
    for (let i = 0; i < n; i += 1) {
      await new Promise((r) => setTimeout(r, 0));
      await tick();
    }
  };
  const press = async (el, label) => {
    const btn = [...el.querySelectorAll('button')].find((b) =>
      b.textContent.replace(/\s+/g, ' ').trim().startsWith(label),
    );
    if (!btn) throw new Error(`no control labelled "${label}"`);
    btn.click();
    await settle();
  };

  it('saves the language that is now true, not the one the editor opened with', async () => {
    let stored = null;
    const saved = [];
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'list_voice_profiles') return Promise.resolve(stored ? [{ ...stored }] : []);
      if (cmd === 'create_voice_profile') {
        stored = { ...ADE };
        return Promise.resolve(ADE.id);
      }
      if (cmd === 'set_stt_language') {
        stored = { ...stored, language: args.language };
        return Promise.resolve({ ...stored });
      }
      if (cmd === 'update_voice_profile') {
        saved.push(args.profile);
        stored = { ...stored, ...args.profile, is_active: true };
        return Promise.resolve({ ...stored });
      }
      if (cmd === 'get_crash_reporting') return Promise.resolve({ enabled: false, dsn: '' });
      return Promise.resolve([]);
    });
    capture.update((s) => ({ ...s, stt: { ...s.stt, language: 'en' } }));

    const Settings = (await import('./views/Settings.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Settings({ target: host, props: {} });
    await settle();

    try {
      // 1. Create the profile through the UI rather than relying on the view's
      //    FIRST `onMount` read. Under jsdom that read races the module mock and
      //    comes back empty through `guardedRead` — the list would then be empty
      //    for a reason with nothing to do with this test. `Add` refreshes it.
      await press(host, 'AI & Detection');
      const name = host.querySelector('input[aria-label="New voice profile name"]');
      expect(name, 'the add-a-profile field was not rendered').toBeTruthy();
      name.value = ADE.name;
      name.dispatchEvent(new Event('input'));
      await settle();
      await press(host, 'Add');

      // 2. Open its editor — as if to nudge the sensitivity dial.
      await press(host, 'Edit');
      const langSel = host.querySelector('#vp-lang');
      expect(langSel, 'the profile editor did not open').toBeTruthy();
      expect(langSel.value).toBe('en');

      // 3. Walk to Scripture & Languages and pin Yoruba. The editor is still open
      //    behind this branch, holding `language: 'en'`.
      await press(host, 'Scripture & Languages');
      const select = host.querySelector('select[aria-label="Recognition language"]');
      expect(select, 'the recognition language select was not rendered').toBeTruthy();
      select.value = 'yo';
      select.dispatchEvent(new Event('change'));
      await settle();
      expect(get(capture).stt.language).toBe('yo');

      // 4. Back to the editor, and save the calibration.
      await press(host, 'AI & Detection');
      await press(host, 'Save profile');

      expect(saved, 'Save profile never reached update_voice_profile').toHaveLength(1);
      expect(saved[0].language, 'the editor put the old language back').toBe('yo');
      // …and the select on the other tab still agrees with what was saved. Nothing
      // on the save path used to touch this store, so the two surfaces could differ.
      expect(get(capture).stt.language).toBe('yo');
    } finally {
      try {
        app?.$destroy();
      } catch {
        /* a view that failed to mount has nothing to destroy */
      }
      host?.remove();
    }
  });

  it('and it follows the profile that is promoted when the active one is DELETED', async () => {
    // The third door, and the one easiest to miss because nobody picks a language
    // on it. Deleting the active profile promotes the next remaining one and
    // applies it live (`main.rs`), so the engine moves to a language the operator
    // never chose while Scripture & Languages goes on showing the deleted
    // profile's. The wrapper's own comment enumerated two doors when there were
    // three. Driven through the store, because the sequence is what matters.
    const { deleteVoiceProfile } = await import('./stores/capture.js');
    invoke.mockReset();
    invoke.mockImplementation((cmd) =>
      cmd === 'delete_voice_profile'
        ? Promise.resolve({ ...ADE, id: 2, name: 'Guest', language: 'sw', is_active: true })
        : Promise.resolve([]),
    );
    capture.update((s) => ({ ...s, stt: { ...s.stt, language: 'yo' } }));

    await deleteVoiceProfile(1);

    expect(get(capture).stt.language).toBe('sw');
  });

  it('and the Scripture select follows a language changed IN the editor', async () => {
    // The same disagreement through the other door. The editor's own Language
    // select writes `voice_profiles.language` and `apply_profile` puts it on the
    // live engine — and `capture.stt.language`, which is all Scripture & Languages
    // renders, was untouched by that path. So the two tabs could sit on different
    // answers to one column, which is what RG-138 is about. `is_active` is taken
    // from what the BACKEND returned, never from the payload's copy of it.
    let stored = null;
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'list_voice_profiles') return Promise.resolve(stored ? [{ ...stored }] : []);
      if (cmd === 'create_voice_profile') {
        stored = { ...ADE };
        return Promise.resolve(ADE.id);
      }
      if (cmd === 'update_voice_profile') {
        // What main.rs returns: the row as written, with is_active stamped from
        // the database rather than echoed from the request.
        stored = { ...stored, ...args.profile, is_active: true };
        return Promise.resolve({ ...stored });
      }
      if (cmd === 'get_crash_reporting') return Promise.resolve({ enabled: false, dsn: '' });
      return Promise.resolve([]);
    });
    capture.update((s) => ({ ...s, stt: { ...s.stt, language: 'en' } }));

    const Settings = (await import('./views/Settings.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Settings({ target: host, props: {} });
    await settle();

    try {
      await press(host, 'AI & Detection');
      const name = host.querySelector('input[aria-label="New voice profile name"]');
      name.value = ADE.name;
      name.dispatchEvent(new Event('input'));
      await settle();
      await press(host, 'Add');
      await press(host, 'Edit');

      const langSel = host.querySelector('#vp-lang');
      expect(langSel, 'the profile editor did not open').toBeTruthy();
      langSel.value = 'ha';
      langSel.dispatchEvent(new Event('change'));
      await settle();
      await press(host, 'Save profile');

      expect(get(capture).stt.language).toBe('ha');
      await press(host, 'Scripture & Languages');
      expect(host.querySelector('select[aria-label="Recognition language"]').value).toBe('ha');
    } finally {
      try {
        app?.$destroy();
      } catch {
        /* a view that failed to mount has nothing to destroy */
      }
      host?.remove();
    }
  });
});
