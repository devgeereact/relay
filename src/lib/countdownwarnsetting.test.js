// THE WARNING THRESHOLD IS A SETTING, AND SOMETHING READS IT.
//
// `layers.js::countdownWarning` carried a comment for the whole life of the
// project saying the threshold was a rule rather than a setting, deliberately,
// because "the control belongs in the Settings pass, and a setting with nowhere
// to set it is worse than a sensible default". Wave 3 track D is that pass.
//
// The defect this file exists to prevent is the OPPOSITE one, and it has already
// happened here once: on 2026-09-10 seven Settings controls were removed because
// each saved a preference nothing read (DECISIONS §69). A control that writes a
// row nobody opens is worse than no control, because it tells an operator their
// countdown will turn red at ninety seconds and then it does not.
//
// So the assertions below are deliberately NOT "the setter calls set_setting".
// They run the real store wrapper, then ask the real warning rule, with nothing
// mocked between the two but the Tauri bridge itself. If the write and the read
// ever come apart, the figure the rule answers with stops moving and these fail.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { countdownWarning, COUNTDOWN_WARN_MS, setCountdownWarnDefault } = await import('./layers.js');
const { countdownWarnMs, loadCountdownWarnMs, setCountdownWarnMs } = await import('./stores/capture.js');

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');
const SETTINGS = read('src/lib/views/Settings.svelte');
const APP = read('src/App.svelte');
/** Comments are not the surface — this repository's own scanning rule. */
const strip = (s) =>
  s
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '');

describe('the countdown warning default', () => {
  beforeEach(() => {
    invoke.mockReset();
    setCountdownWarnDefault(COUNTDOWN_WARN_MS);
  });

  it('ships as the last minute, so a church that never opens Settings sees no change', () => {
    expect(COUNTDOWN_WARN_MS).toBe(60_000);
    expect(countdownWarning(59_000, 15 * 60_000)).toBe(true);
    expect(countdownWarning(61_000, 15 * 60_000)).toBe(false);
  });

  it('saving the setting changes when the wall warns, not just what is in the row', async () => {
    invoke.mockResolvedValue(null);
    // Ninety seconds left on a fifteen-minute countdown: not a warning today.
    expect(countdownWarning(90_000, 15 * 60_000)).toBe(false);
    await setCountdownWarnMs(120_000);
    // The row was written …
    expect(invoke).toHaveBeenCalledWith('set_setting', {
      key: 'countdown.warn_ms',
      value: '120000',
    });
    // … and THIS is the half §69 was about. The rule the wall, the stage page and
    // the dock all ask now answers differently.
    expect(countdownWarning(90_000, 15 * 60_000)).toBe(true);
    expect(get(countdownWarnMs)).toBe(120_000);
  });

  it('a console reopened mid-service reads the figure back and warns at it', async () => {
    invoke.mockResolvedValue('120000');
    await loadCountdownWarnMs();
    expect(invoke).toHaveBeenCalledWith('get_setting', { key: 'countdown.warn_ms' });
    expect(countdownWarning(90_000, 15 * 60_000)).toBe(true);
    expect(get(countdownWarnMs)).toBe(120_000);
  });

  it('an unset or unreadable row is the shipped default, never zero', async () => {
    // A window of zero is a warning colour that never comes on. Both the absent
    // row and a backend that cannot answer must land on the minute.
    for (const answer of [null, '', 'soon', '0', '-5']) {
      setCountdownWarnDefault(120_000);
      invoke.mockReset();
      invoke.mockResolvedValue(answer);
      await loadCountdownWarnMs();
      expect(get(countdownWarnMs), `raw ${JSON.stringify(answer)}`).toBe(COUNTDOWN_WARN_MS);
      expect(countdownWarning(59_000, 15 * 60_000)).toBe(true);
      expect(countdownWarning(90_000, 15 * 60_000)).toBe(false);
    }
    setCountdownWarnDefault(120_000);
    invoke.mockReset();
    invoke.mockRejectedValue('no backend');
    await loadCountdownWarnMs();
    expect(get(countdownWarnMs)).toBe(COUNTDOWN_WARN_MS);
    expect(countdownWarning(90_000, 15 * 60_000)).toBe(false);
  });

  it('the default is still scaled by the short-countdown rule, and a chosen figure is not', () => {
    setCountdownWarnDefault(120_000);
    // A two-minute countdown does not warn for its whole life just because the
    // DEFAULT went up — the tenth rule is about a figure nobody chose per timer.
    expect(countdownWarning(90_000, 2 * 60_000)).toBe(false);
    expect(countdownWarning(11_000, 2 * 60_000)).toBe(true);
    // A figure chosen for THIS timer still overrides both.
    expect(countdownWarning(90_000, 2 * 60_000, 100_000)).toBe(true);
  });

  it('the default is applied at launch, not only when Settings is opened', () => {
    // The dock and the console's programme pane read the rule long before anyone
    // visits Settings. If the loader only ran on that page, an operator who set
    // ninety seconds would get sixty for the whole of the next service.
    const app = strip(APP);
    expect(app).toMatch(/loadCountdownWarnMs/);
    expect(app.slice(app.indexOf('onMount('))).toMatch(/loadCountdownWarnMs\(\)/);
  });
});

describe('the control an operator actually sets it with', () => {
  const code = strip(SETTINGS);
  const markup = code.slice(code.indexOf('</script>'), code.indexOf('<style>'));

  it('is on Getting started, with the other number a new install sets once', () => {
    // It was on General, beside Service length, on the argument that the two are
    // the figures a service is timed by. General is gone: with safe mode promoted
    // to Before the service and service length moved to This room it held one
    // switch and a paragraph. The two rows parted because they are used at
    // different times — a planned length is a fact about the morning and belongs
    // with the room, a countdown warning is set once and never touched again.
    const start = markup.slice(markup.indexOf("section === 'start'"));
    expect(start).toMatch(/Countdown warning/);
    expect(start).toMatch(/setCountdownWarnMs/);
  });

  it('shows the figure that is in force rather than a local mirror of it', () => {
    // Rule 35 in miniature: a control that renders its own copy can show a value
    // the store does not hold, and then the page and the wall disagree.
    expect(markup).toMatch(/\$countdownWarnMs/);
    expect(code).toMatch(/import \{[^}]*countdownWarnMs[^}]*\} from '\.\.\/stores\/capture\.js'/s);
  });

  it('is named, so it is reachable without a mouse and countable by qa-inventory', () => {
    const at = markup.indexOf('setCountdownWarnMs');
    const row = markup.slice(markup.lastIndexOf('<div class="rw-nv">', at), markup.indexOf('</div>', at));
    expect(row).toMatch(/aria-label="[^"]+"/);
  });

  it('asks no native confirm, alert or prompt (rule 41)', () => {
    const at = code.indexOf('setCountdownWarnMs');
    const near = code.slice(Math.max(0, at - 2000), at + 2000);
    expect(near).not.toMatch(/\b(confirm|alert|prompt)\s*\(/);
  });
});
