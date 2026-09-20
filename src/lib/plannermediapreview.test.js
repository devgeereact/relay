// A MEDIA CUE IN THE PLANNER SHOWS THE PICTURE, NOT ITS FILENAME.
//
// The Planner's cue inspector exists to answer one question — what does this put
// on the wall? — and for the one cue type whose whole content IS a picture it
// answered with a sentence: "The slide is the picture — media plays full-frame."
// True, and useless: an operator building a Tuesday plan could not tell a
// background they meant from one they picked by mistake, and the filename is the
// only thing they had to go on.
//
// `TemplateRender` could already paint it. It branches on `content.media_kind`
// in both layer and legacy mode and reads `content.media_url`. Nothing ever
// handed it either field for a plan cue, so the path died four links upstream:
//
//   1 · `plan.js::slidesOf` returned `{ tag: 'BG', label, text: '' }` — no id, no URL.
//   2 · `ServicePlanner`'s `previewContent` was built without `media_url`/`media_kind`.
//   3 · empty text took `previewState`'s media branch and returned `plate: false`.
//   4 · `{#if pv.plate}` was false, so the renderer was never reached and
//       `.sp-noslide` printed the sentence instead.
//
// THE MODEL IS THE ONE THAT ALREADY EXISTS, deliberately. `mediaUrl(host, asset)`
// in `bundledbackgrounds.js` is the shared builder and mirrors `main.rs::media_url`
// — two doors, one rule, including the `bundled:` case where a seeded picture has
// no file under `/media/<id>` at all (DECISIONS §90). The Library's media pane is
// the existing second door; the Planner is now the third and calls the same
// function rather than rebuilding the URL. A hand-rolled copy here would be the
// exact bug that module's doc comment was written about.
//
// AND THE VERDICT STAYS IN `plan.js`. `previewState` already owns which of four
// things the preview may honestly claim, "so the four situations it separates are
// testable without a component". A missing asset is a FIFTH, and it belongs in the
// same place rather than as an `{#if}` in the view.
//
// ── HOW EACH TEST WAS CHECKED ────────────────────────────────────────────────
//
// Test the bug, not the fix. Every case below was watched to go RED against the
// pre-fix code — `previewState(item, hasText)` with no media argument, and
// `previewContent` without the two media fields:
//
//   · "plan.js · a resolved asset" — returns `plate: false` before the fix.
//   · "plan.js · a missing asset" — returns the self-drawn sentence, which says
//     the cue is fine, for a cue that would put nothing on a screen.
//   · "the inspector paints the picture" / "…a video" — no `<img>`/`<video>` is
//     in the DOM at all before the fix; `.sp-noslide` is.
//   · "a deleted asset falls back and names it" — before the fix this passed for
//     the WRONG reason (the sentence was always shown), so it also asserts the
//     filename is named and that no broken `<img>` was rendered. Those two halves
//     are what make it a test of this change.
//   · "no media cue is unaffected" and "a media cue with no id" — GREEN both
//     ways, on purpose: they catch the opposite mistake, a change that forced the
//     plate on for every cue or crashed on a payload with nothing to look up.
//   · "one URL builder" — reads the view off disk. It is what makes this a claim
//     about following the existing model rather than about the literal string.
//
//   npx vitest run src/lib/plannermediapreview.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const { clearSession } = await import('./session.js');
const { previewState } = await import('./plan.js');

// The same self-detecting gate `cuetimer.test.js` and `surface.test.js` use:
// without `resolve: { conditions: ['browser'] }` in `vitest.config.js`, `onMount`
// is a literal empty function and a mounted view fetches nothing. These SKIP
// loudly rather than pass over a Planner that never loaded a plan.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const settle = (ms = 40) => new Promise((r) => setTimeout(r, ms));
async function until(predicate, what, tries = 60) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
    await tick();
  }
  throw new Error(`timed out waiting for: ${what}`);
}

const PLAN = { id: 1, title: 'Sunday Morning', plan_date: '2026-09-20', cue_count: 1 };

/** A row as `list_media` returns it. */
const PHOTO = { id: 7, filename: 'sunrise.jpg', kind: 'image', path: 'media/7' };
const CLIP = { id: 8, filename: 'worship-loop.mp4', kind: 'video', path: 'media/8' };
/** A picture Relay SHIPS: no file under `/media/<id>` at all (DECISIONS §90). */
const SEEDED = { id: 9, filename: 'Dawn.jpg', kind: 'image', path: 'bundled:bg/dawn.jpg' };

const mediaCue = (payload, over = {}) => ({
  id: 31,
  plan_id: 1,
  position: 0,
  cue_type: 'media',
  label: payload.filename || 'Background',
  payload_json: JSON.stringify(payload),
  template_id: null,
  section_title: '',
  duration_sec: 0,
  timer_minutes: null,
  ...over,
});

let cues = [];
let library = [];
let host;
let app;

beforeEach(() => {
  cues = [mediaCue({ media_id: PHOTO.id, kind: PHOTO.kind, filename: PHOTO.filename })];
  library = [PHOTO, CLIP, SEEDED];
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_plans':
        return Promise.resolve([PLAN]);
      case 'plan_items':
        return Promise.resolve(cues);
      case 'list_templates':
        return Promise.resolve([{ id: 1, name: 'Classic Serif' }]);
      case 'list_media':
        return Promise.resolve(library);
      case 'local_ip':
        return Promise.resolve('192.168.1.50');
      case 'list_output_channels':
      case 'list_announcements':
      case 'search_scripture':
      case 'search_songs':
      case 'list_arrangements':
      case 'list_books':
        return Promise.resolve([]);
      case 'get_rehearsal':
        return Promise.resolve(false);
      case 'get_sensitivity':
        return Promise.resolve(50);
      default:
        return Promise.resolve(null);
    }
  });
  cap.capture.update((c) => ({ ...c, available: true }));
  cap.rehearsing.set(false);
  clearSession();
  host = document.createElement('div');
  document.body.appendChild(host);
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  clearSession();
  document.body.innerHTML = '';
});

/** Open the plan and select its first cue. */
async function openFirstCue() {
  const ServicePlanner = (await import('./views/ServicePlanner.svelte')).default;
  app = new ServicePlanner({ target: host });
  await until(() => host.querySelector('.sp-railcard'), 'the plan rail');
  host.querySelector('.sp-railcard').click();
  await until(() => host.querySelector('.sp-row'), 'the running order');
  host.querySelector('.sp-row').click();
  await until(() => host.querySelector('.sp-preview, .sp-noslide'), 'the inspector preview');
}

// ─────────────────────────────────────────────────────────────────────────────
// 1 · THE VERDICT, WHERE THE OTHER FOUR LIVE
// ─────────────────────────────────────────────────────────────────────────────
describe('plan.js owns whether a media cue has something to show', () => {
  const c = { cue_type: 'media' };

  it('a resolved asset is a slide to render, over the plate', () => {
    const v = previewState(c, false, { found: true, filename: 'sunrise.jpg' });
    expect(v.state).toBe('render');
    expect(v.plate).toBe(true);
  });

  it('a missing asset is a DEFECT that names the file, not a kind behaving well', () => {
    const v = previewState(c, false, { found: false, filename: 'sunrise.jpg' });
    expect(v.plate).toBe(false);
    expect(v.state).toBe('empty');
    expect(v.message).toContain('sunrise.jpg');
    // It must NOT read like the self-drawn sentence, which says all is well.
    expect(v.message).not.toMatch(/plays full-frame/);
  });

  it('with nothing looked up, the old four verdicts are untouched', () => {
    // GREEN either way, deliberately: the new argument is additive, and a change
    // that altered the existing answers would be a worse bug than the one fixed.
    expect(previewState(c, false)).toMatchObject({ state: 'self', plate: false });
    expect(previewState({ cue_type: 'countdown' }, false)).toMatchObject({ state: 'self' });
    expect(previewState({ cue_type: 'scripture' }, false)).toMatchObject({ state: 'empty' });
    expect(previewState(null, false)).toMatchObject({ state: 'none' });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2 · WHAT THE OPERATOR ACTUALLY SEES
// ─────────────────────────────────────────────────────────────────────────────
describe('the Planner inspector paints a media cue', () => {
  itMounted('a picture is rendered, at the URL an output screen would use', async () => {
    await openFirstCue();
    await until(() => host.querySelector('.sp-preview img'), 'the thumbnail');

    const img = host.querySelector('.sp-preview img');
    // The SHARED builder's answer — the app's HTTP server on 8032, never Vite.
    expect(img.getAttribute('src')).toBe('http://192.168.1.50:8032/media/7');
    // And the sentence that stood in for it is gone.
    expect(host.querySelector('.sp-noslide')).toBeNull();
  });

  itMounted('a video is rendered as a video, with metadata only', async () => {
    cues = [mediaCue({ media_id: CLIP.id, kind: CLIP.kind, filename: CLIP.filename })];
    await openFirstCue();
    await until(() => host.querySelector('.sp-preview video'), 'the video thumbnail');

    const vid = host.querySelector('.sp-preview video');
    expect(vid.getAttribute('src')).toBe('http://192.168.1.50:8032/media/8');
    // MUTED AND INLINE — a plan being built on a Tuesday may not make a noise.
    expect(vid.muted).toBe(true);
    expect(vid.hasAttribute('playsinline')).toBe(true);
  });

  // WHY NOT `preload="metadata"` ABOVE, since the Library's cards use it.
  //
  // There are two media-rendering models in this repo and they are not the same
  // model. A Library CARD is a hand-rolled thumbnail — `VerseDeck` writes its own
  // `<img>`/`<video preload="metadata" muted playsinline>` because a card is a
  // picture OF a slide. The Planner's inspector preview is the other kind: it goes
  // through `TemplateRender`, over the chequered plate, under a caption that says
  // "Rendered by the same engine as the output screens, so this is what the wall
  // will show". CLAUDE.md is explicit that `TemplateRender` is the ONE renderer,
  // and a second `<video>` hand-rolled here would both fork it and make that
  // caption false — a preview that no longer matches the wall is the failure the
  // whole WYSIWYG-by-construction rule exists to prevent.
  //
  // So this went through the renderer, which paints video `autoplay loop muted
  // playsinline` and sets no `preload`. Adding the attribute upstream would be
  // decoration rather than a fix: `preload` is ignored whenever `autoplay` is
  // present, because the browser must fetch the file to autoplay it at all. The
  // two attributes that carry the actual guarantee — silence, and no fullscreen
  // takeover — are asserted above and `TemplateRender` already keeps both.
  it('the preview is the ONE renderer, not a second hand-rolled thumbnail', () => {
    const view = readFileSync(resolve(__dirname, 'views/ServicePlanner.svelte'), 'utf8');
    // No hand-rolled media element in the Planner at all: the renderer owns it.
    expect(view).not.toMatch(/<video\b/);
    expect(view).not.toMatch(/<img\b/);
  });

  itMounted('a picture Relay ships resolves through the bundled rule, not /media/<id>', async () => {
    cues = [mediaCue({ media_id: SEEDED.id, kind: SEEDED.kind, filename: SEEDED.filename })];
    await openFirstCue();
    await until(() => host.querySelector('.sp-preview img'), 'the seeded thumbnail');

    // `/media/9` does not exist for this row. Getting this wrong is a broken
    // image box for every background Relay ships.
    expect(host.querySelector('.sp-preview img').getAttribute('src')).toBe(
      'http://192.168.1.50:8032/bg/dawn.jpg',
    );
  });

  itMounted('a DELETED asset falls back to words and says which file is gone', async () => {
    // The row was removed from the library; the cue still points at it.
    library = [CLIP, SEEDED];
    await openFirstCue();
    await until(() => host.querySelector('.sp-noslide'), 'the fallback sentence');

    // No broken <img>: a 404 box in a preview claiming to be what the wall shows
    // is worse than a sentence.
    expect(host.querySelector('.sp-preview img')).toBeNull();
    expect(host.querySelector('.sp-preview video')).toBeNull();
    // And it names the asset, so the operator knows what to re-add.
    expect(host.querySelector('.sp-noslide').textContent).toContain('sunrise.jpg');
  });

  itMounted('a media cue whose payload names no asset does not crash the inspector', async () => {
    // GREEN both ways. An older or hand-edited payload has no `media_id`; the
    // inspector must still open and still say something honest.
    cues = [mediaCue({})];
    await openFirstCue();
    await until(() => host.querySelector('.sp-noslide'), 'the fallback sentence');
    expect(host.querySelector('.sp-noslide').textContent.trim()).toBeTruthy();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3 · ONE URL BUILDER, NOT A SECOND ONE
// ─────────────────────────────────────────────────────────────────────────────
describe('the Planner uses the shared URL builder', () => {
  const view = readFileSync(resolve(__dirname, 'views/ServicePlanner.svelte'), 'utf8');

  it('calls mediaUrl rather than rebuilding the URL by hand', () => {
    expect(view).toMatch(/import\s*\{[^}]*\bmediaUrl\b[^}]*\}\s*from\s*'[^']*bundledbackgrounds/);
    expect(view).toMatch(/mediaUrl\(/);
    // A hand-rolled `http://${host}:8032/media/...` here is the bug the shared
    // module's doc comment was written about — it would silently break every
    // bundled picture, which has no file under `/media/<id>`.
    expect(view).not.toMatch(/:8032\/media\//);
  });
});
