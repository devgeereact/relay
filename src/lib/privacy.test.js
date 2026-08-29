// RG-17 — what is leaving this machine, answered from the live settings.
//
// Relay's privacy story is the strongest thing about it and it has been invisible:
// it lives in `PRIVACY.md`, which nobody in a booth reads. The screen exists so an
// operator can check rather than trust.
//
// Two rules, and the second is the one that makes it worth having:
//
//   1. Every row is read from the ACTUAL state. A page that says "off" because
//      somebody typed "off" is worth less than no page.
//   2. It states the unflattering half in the same size type. A privacy page that
//      lists only the reassuring facts is an advert.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');
const settings = read('src/lib/views/Settings.svelte');
const privacy = settings.slice(
  settings.indexOf("section === 'privacy'"),
  settings.indexOf("section === 'advanced'"),
);

describe('nothing on it is hardcoded', () => {
  it('crash reporting reads the live setting, not a literal', () => {
    // The one thing Relay can send, and the one row somebody opens this page to
    // check. It is derived from the value the backend returned.
    expect(settings).toMatch(/\$: crashOn = !!crash\.enabled;/);
    expect(privacy).toMatch(/\{crashOn\s*\n?\s*\?/);
    // …and it must not be a fixed string.
    expect(privacy).not.toMatch(/Crash reporting<\/span>\s*<span[^>]*>\s*(ON|OFF)\b/);
  });

  it('speech recognition reflects whether a model is actually loaded', () => {
    expect(privacy).toMatch(/\$capture\.stt\.loaded/);
    expect(privacy).toMatch(/No model loaded — nothing is being transcribed/);
  });

  it('names this machine’s real LAN address rather than a placeholder', () => {
    expect(privacy).toMatch(/\{lanIp \|\| 'this computer'\}:8032/);
  });
});

describe('it says the unflattering half too', () => {
  it('states that anyone on the WiFi can CHANGE what is on the screens', () => {
    // The preacher's remote has no password, by design (DECISIONS §35). A privacy
    // page that mentioned only "they can see it" would be misleading by omission,
    // which is the failure PRIVACY.md was corrected for in the first place.
    expect(privacy).toMatch(/can\s*<b>change it<\/b>|<b>and can\s*\n?\s*change it<\/b>/);
    expect(privacy).toMatch(/no password/);
  });

  it('bounds the exposure honestly — the LAN cannot reach the history', () => {
    expect(privacy).toMatch(/cannot\s*\n?\s*reach your transcripts, plans or history/);
  });

  it('points at the full account rather than pretending to be it', () => {
    expect(privacy).toMatch(/PRIVACY\.md/);
    expect(privacy).toMatch(/§35/);
  });
});

describe('it is a report, not a control panel', () => {
  it('changes nothing — every setting lives where it already lived', () => {
    // A second place to toggle crash reporting is a second answer to one question.
    expect(privacy).toMatch(/it is a report on the settings you have/);
    expect(privacy).not.toMatch(/on:click=|on:change=/);
  });

  it('is reachable as its own section', () => {
    expect(settings).toMatch(/key: 'privacy'/);
  });
});
