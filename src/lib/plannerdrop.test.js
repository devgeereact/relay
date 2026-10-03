// DRAG MEDIA STRAIGHT INTO THE PLANNER.
//
// There was no file drop anywhere in `src/` before this — zero
// `dataTransfer.files`, and no `tauri://drag-drop`, `dragDropEnabled` or
// `onDragDropEvent` in `src-tauri/` either. Putting a picture in a plan meant
// Library → Import → file dialog → the media review sheet → back to Planner →
// Add cue → search for the filename → click. Seven steps.
//
// ── WHAT THIS FILE HOLDS ────────────────────────────────────────────────────
//
//   1 · the triage — what becomes a cue, and what is refused with a SENTENCE
//       rather than a silent nothing;
//   2 · A DOCUMENT IS REFUSED AT THE DROP, not at the fire. `main.rs` answers
//       "documents can't be shown as an output background yet" from both
//       `fire_media` and `show_background`, so a dropped PDF that became a cue
//       would look exactly like a working one and fail on a Sunday, in front of
//       people, at the moment it was reached;
//   3 · the size ceiling is the one that already exists — `fileToBase64` holds
//       `MAX_IMPORT_BYTES` and throws before allocating, because the allocation
//       IS the failure. The drop path must not re-implement it, or there are two
//       ceilings and one of them will drift;
//   4 · where the cue will land, SHOWN before the mouse is released, and shown
//       from the same number the insertion afterwards uses;
//   5 · that it cannot collide with the running order's pointer reorder, which
//       was deliberately migrated off HTML5 drag;
//   6 · that a dropped file produces a cue which PREVIEWS correctly — the payload
//       shape `plannermediapreview.test.js` pins.
//
// ── HOW EACH WAS CHECKED ────────────────────────────────────────────────────
//
// Test the bug, not the fix. Each was watched red against a defect the obvious
// implementation has:
//
//   · a PDF accepted as a cue, which is the failure mode this feature could most
//     easily ship with — it imports fine, it previews as a media cue, and it dies
//     at `fire_media`.
//   · a mixed drop refused whole, so the operator drags the pictures again.
//   · `dragover` without `preventDefault`, which navigates the webview to the
//     file and takes the console with it.
//   · an insertion index computed in the handler AND again at the insert, so the
//     marker promises a position the reorder does not deliver.
//   · a cue built without `media_id`, which previews as a filename.
//
//   npx vitest run src/lib/plannerdrop.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  IMAGE_EXT,
  VIDEO_EXT,
  DOC_EXT,
  extOf,
  kindOfFile,
  triageDrop,
  refusalMessage,
  dropGapAt,
  orderWithDropAt,
  droppedMessage,
} from './plannerdrop.js';
import { previewState, slidesOf } from './plan.js';
import { mediaCuePayload } from './planneradd.js';
// ONE STRIPPER, SHARED — `colourlaw.test.js` and `names.test.js` both record why
// a hand-rolled regex is not good enough here. It also matters more than usual in
// this file: the module and the view BOTH explain, in prose, the thing they must
// not do ("this was HTML5 drag-and-drop (`draggable`, …)", "`fileToBase64` holds
// MAX_IMPORT_BYTES"). A scanner that counted those would fail on a correct file,
// and the cheapest way to go green would be deleting the explanation — which is
// the more valuable half.
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const { clearSession } = await import('./session.js');
const { MAX_IMPORT_BYTES } = cap;

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

const SRC = resolve(process.cwd(), 'src');
const PLANNER = readFileSync(resolve(SRC, 'lib/views/ServicePlanner.svelte'), 'utf8');
const LIBRARY = readFileSync(resolve(SRC, 'lib/views/Library.svelte'), 'utf8');

/** A `File` the drop path will accept the shape of. */
function fakeFile(name, bytes = 8, type = '') {
  const blob = new Blob([new Uint8Array(bytes)], { type });
  return new File([blob], name, { type });
}

// ─────────────────────────────────────────────────────────────────────────────
// 1 · WHAT RELAY TAKES, AND THE ONE LIST IT IS READ FROM
// ─────────────────────────────────────────────────────────────────────────────
describe('what a dropped file is', () => {
  it('the kinds agree with Library.svelte, which already routes on them', () => {
    // Two copies of "what Relay accepts" is how a file becomes importable in one
    // place and invisible in the other — the exact defect `Library.svelte` records
    // against its own file picker, whose accept list was once shorter than its
    // router's (".bmp, .avif, .svg, .mkv, .m4v, .pro5 and .key were greyed out in
    // the dialog even though the importer handles them").
    const listIn = (name) => {
      const m = LIBRARY.match(new RegExp(`const ${name} = \\[([^\\]]*)\\]`));
      expect(m, `${name} not found in Library.svelte`).toBeTruthy();
      return m[1].split(',').map((s) => s.trim().replace(/^'|'$/g, '')).filter(Boolean);
    };
    expect(IMAGE_EXT).toEqual(listIn('IMG'));
    expect(VIDEO_EXT).toEqual(listIn('VID'));
    expect(DOC_EXT).toEqual(listIn('DOC'));
  });

  it('an extension is read off the end of the name, case-insensitively', () => {
    expect(extOf('sunrise.JPG')).toBe('jpg');
    expect(extOf('my.holiday.photo.png')).toBe('png');
    expect(extOf('README')).toBe('');
    // A dotfile is not an extension: `.gitignore` is a name, not a `gitignore` file.
    expect(extOf('.gitignore')).toBe('');
  });

  it('pictures and video are cues; documents are named as documents', () => {
    expect(kindOfFile('a.png')).toBe('image');
    expect(kindOfFile('a.MOV')).toBe('video');
    expect(kindOfFile('a.pdf')).toBe('document');
    expect(kindOfFile('a.zip')).toBeNull();
    expect(kindOfFile('')).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2 · A DOCUMENT IS REFUSED AT THE DROP, NOT AT THE FIRE
// ─────────────────────────────────────────────────────────────────────────────
describe('a dropped file that cannot be fired must not become a cue', () => {
  it('a PDF is refused, and the sentence says what to do with it instead', () => {
    // THE FAILURE THIS FEATURE COULD MOST EASILY HAVE SHIPPED WITH. A PDF imports
    // fine and previews as a media cue; it dies at `fire_media`, on a Sunday.
    const { accept, refused } = triageDrop([fakeFile('notices.pdf')]);
    expect(accept).toEqual([]);
    expect(refused).toHaveLength(1);
    expect(refused[0].reason).toContain('document');
    expect(refused[0].reason).toContain('notices.pdf');
    // Where it DOES belong. "Unsupported" tells a volunteer nothing they can act on.
    expect(refused[0].reason).toContain('Library');
  });

  it('every document extension the Library takes is refused as a cue', () => {
    for (const ext of DOC_EXT) {
      const { accept, refused } = triageDrop([fakeFile(`deck.${ext}`)]);
      expect(accept, ext).toEqual([]);
      expect(refused[0].reason, ext).toContain('document');
    }
  });

  it('a kind Relay does not import names the extension', () => {
    const { refused } = triageDrop([fakeFile('service.zip')]);
    expect(refused[0].reason).toContain('.zip');
    expect(refused[0].reason).toContain('png');
  });

  it('a mixed drop is NOT all-or-nothing — the pictures land and the PDF is named', () => {
    // Refusing the batch over one file makes an operator drag the rest again;
    // dropping the PDF silently is worse still.
    const { accept, refused } = triageDrop([
      fakeFile('one.jpg'),
      fakeFile('notices.pdf'),
      fakeFile('loop.mp4'),
    ]);
    expect(accept.map((a) => a.kind)).toEqual(['image', 'video']);
    expect(refused.map((r) => r.name)).toEqual(['notices.pdf']);
  });

  it('several refusals are counted, and the first reason is given', () => {
    // A bar that grew a line per file would push the running order off the pane.
    const many = triageDrop([fakeFile('a.pdf'), fakeFile('b.pdf'), fakeFile('c.zip')]);
    const said = refusalMessage(many.refused);
    expect(said).toContain('3 files were not added');
    expect(said).toContain('a.pdf');
    expect(refusalMessage([])).toBe('');
  });

  it('a drop with nothing in it does not throw', () => {
    expect(triageDrop(null)).toEqual({ accept: [], refused: [] });
    expect(triageDrop([])).toEqual({ accept: [], refused: [] });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3 · THE SIZE CEILING IS THE ONE THAT ALREADY EXISTS
// ─────────────────────────────────────────────────────────────────────────────
describe('a file that is too large is refused with a sentence, by the existing guard', () => {
  it('fileToBase64 throws before it allocates, and says the two sizes', async () => {
    // The allocation IS the failure: four copies of the file exist at the peak,
    // and on a church laptop that is the OS killing Relay with nothing in any log.
    const big = { name: 'service-4k.mp4', size: MAX_IMPORT_BYTES + 1 };
    await expect(cap.fileToBase64(big)).rejects.toThrow(/service-4k\.mp4/);
    await expect(cap.fileToBase64(big)).rejects.toThrow(/256 MB/);
  });

  it('and the drop path does NOT re-implement it', () => {
    // Two ceilings is one ceiling that will drift. `MAX_IMPORT_BYTES` appears in
    // the store and in `main.rs`, and nowhere in the drop module or the view.
    const drop = codeOnly(readFileSync(resolve(SRC, 'lib/plannerdrop.js'), 'utf8'));
    expect(drop).not.toMatch(/MAX_IMPORT_BYTES|1024 \* 1024/);
    expect(codeOnly(PLANNER)).not.toMatch(/MAX_IMPORT_BYTES/);
    // …it goes through the choke point instead.
    expect(codeOnly(PLANNER)).toMatch(/fileToBase64\(/);
    // Guards the guard: the stripper must not have blanked the whole file.
    expect(drop).toMatch(/export function triageDrop/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4 · WHERE IT WILL LAND, SHOWN BEFORE THE MOUSE IS RELEASED
// ─────────────────────────────────────────────────────────────────────────────
describe('the gap the operator is shown is the gap the cue goes into', () => {
  const rows = [
    { top: 0, bottom: 30 },
    { top: 30, bottom: 60 },
    { top: 60, bottom: 90 },
  ];

  it('a row’s MIDPOINT is the boundary, as it is in every list', () => {
    expect(dropGapAt(rows, 5)).toBe(0);
    expect(dropGapAt(rows, 14)).toBe(0);
    expect(dropGapAt(rows, 16)).toBe(1);
    expect(dropGapAt(rows, 44)).toBe(1);
    expect(dropGapAt(rows, 46)).toBe(2);
    expect(dropGapAt(rows, 200)).toBe(3);
  });

  it('an empty list is the one gap it has', () => {
    expect(dropGapAt([], 40)).toBe(0);
    expect(dropGapAt(null, 40)).toBe(0);
  });

  it('unlaid-out geometry is the END of the list, deterministically', () => {
    // Every rect reports zero in a pane mid-layout, and always in jsdom.
    // Answering "after everything" is what `add_plan_item` does on its own, so a
    // drop onto an unmeasured list behaves exactly as a plain add rather than
    // landing somewhere arbitrary.
    const flat = [{ top: 0, bottom: 0 }, { top: 0, bottom: 0 }];
    expect(dropGapAt(flat, 120)).toBe(2);
    expect(dropGapAt(rows, NaN)).toBe(0);
  });

  it('the order it produces puts the new cues in that gap', () => {
    // `add_plan_item` appends, so the new ids arrive at the end and are lifted out.
    expect(orderWithDropAt([1, 2, 3, 90], [90], 0)).toEqual([90, 1, 2, 3]);
    expect(orderWithDropAt([1, 2, 3, 90], [90], 1)).toEqual([1, 90, 2, 3]);
    expect(orderWithDropAt([1, 2, 3, 90], [90], 3)).toEqual([1, 2, 3, 90]);
  });

  it('several files land in the gap together, in the order they were dropped', () => {
    expect(orderWithDropAt([1, 2, 90, 91], [90, 91], 1)).toEqual([1, 90, 91, 2]);
  });

  it('a gap past either end is clamped rather than losing the cue', () => {
    expect(orderWithDropAt([1, 2, 90], [90], 99)).toEqual([1, 2, 90]);
    expect(orderWithDropAt([1, 2, 90], [90], -5)).toEqual([90, 1, 2]);
  });

  it('an id that is not in the plan is dropped, not invented', () => {
    expect(orderWithDropAt([1, 2, 3], [90], 1)).toEqual([1, 2, 3]);
    expect(orderWithDropAt(null, [90], 0)).toEqual([]);
  });

  it('the report afterwards counts what actually landed', () => {
    expect(droppedMessage(1)).toBe('Added 1 media cue from the file you dropped.');
    expect(droppedMessage(3)).toContain('Added 3 media cues');
    expect(droppedMessage(0)).toBe('');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5 · IT CANNOT COLLIDE WITH THE REORDER, AND IT CANNOT EAT THE WEBVIEW
// ─────────────────────────────────────────────────────────────────────────────
describe('the drop and the running order’s own drag stay out of each other’s way', () => {
  it('dragover calls preventDefault — without it the webview navigates to the file', () => {
    // The console simply disappears, mid-build, with no way back but a relaunch.
    const fn = PLANNER.slice(PLANNER.indexOf('function onFileOver'));
    const body = fn.slice(0, fn.indexOf('\n  }'));
    expect(body).toMatch(/e\.preventDefault\(\)/);
  });

  it('a drag carrying no FILES is ignored, so a row in hand is untouched', () => {
    const fn = PLANNER.slice(PLANNER.indexOf('function onFileOver'));
    const body = fn.slice(0, fn.indexOf('\n  }'));
    expect(body).toMatch(/dragHasFiles\(e\)/);
    // …and a reorder in progress holds the marker off entirely.
    expect(body).toMatch(/if \(!openPlan \|\| drag \|\|/);
  });

  it('the running order’s reorder is still pointer-based, not HTML5 drag', () => {
    // It was migrated off `draggable`/`dragstart` deliberately, for three recorded
    // reasons. A drop handler is not a licence to put it back.
    // Comments stripped: this file EXPLAINS that it used to be HTML5 drag, and a
    // scanner that read the explanation would make deleting it the way to go green.
    const code = codeOnly(PLANNER);
    expect(code).toMatch(/onGripDown/); // guards the guard
    expect(code).not.toMatch(/\bdraggable\b/);
    expect(code).not.toMatch(/on:dragstart/);
    expect(code).toMatch(/on:pointerdown=\{\(e\) => onGripDown/);
  });

  it('the drop zone is not a control, and takes nothing away from the keyboard', () => {
    // Everything it does is reachable through ＋ Add cue → the media search, which
    // is the path it is a shortcut for. A drop may not be a keyboard's only route.
    expect(PLANNER).toMatch(/class="rw-panebody sp-tablewrap" class:dropping/);
    expect(PLANNER).toMatch(/role="presentation"[\s\S]{0,80}on:dragenter/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 6 · A DROPPED FILE PRODUCES A CUE THAT PREVIEWS
// ─────────────────────────────────────────────────────────────────────────────
describe('the cue a drop builds is the cue the inspector can paint', () => {
  it('it carries media_id and kind, so the preview is the picture not the filename', () => {
    // The payload shape `plannermediapreview.test.js` pins, reached through the
    // same builder the Add panel uses — a fifth way in that cannot disagree with
    // the other four.
    const asset = { id: 7, filename: 'sunrise.jpg', kind: 'image', path: 'media/7' };
    const built = mediaCuePayload(asset);
    const item = { cue_type: 'media', label: built.label, payload_json: JSON.stringify(built.payload) };
    expect(slidesOf(item)[0].media_id).toBe(7);
    expect(slidesOf(item)[0].media_kind).toBe('image');
    expect(previewState(item, false, { found: true, filename: 'sunrise.jpg' }).state).toBe('render');
    expect(previewState(item, false, { found: true, filename: 'sunrise.jpg' }).plate).toBe(true);
  });

  it('a dropped VIDEO stays a video', () => {
    const built = mediaCuePayload({ id: 8, filename: 'loop.mp4', kind: 'video' });
    expect(slidesOf({ cue_type: 'media', payload_json: JSON.stringify(built.payload) })[0].media_kind).toBe('video');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 7 · DRIVEN — A REAL DROP ON A MOUNTED PLANNER
// ─────────────────────────────────────────────────────────────────────────────
const PLAN = { id: 1, title: 'Sunday Morning', plan_date: '2026-09-20', cue_count: 3 };

const planCue = (id) => ({
  id,
  plan_id: 1,
  position: id,
  cue_type: 'scripture',
  label: `Cue ${id}`,
  payload_json: JSON.stringify({ reference: `Romans 8:${id}`, text: 'words' }),
  template_id: null,
  section_title: '',
  duration_sec: 0,
  timer_minutes: null,
  channels_json: null,
});

let cues = [];
let imported = [];
let reorders = [];
let nextMedia = 40;
let nextCue = 90;
let host;
let app;

beforeEach(() => {
  cues = [1, 2, 3].map(planCue);
  imported = [];
  reorders = [];
  nextMedia = 40;
  nextCue = 90;
  invoke.mockReset();
  invoke.mockImplementation((cmd, args) => {
    switch (cmd) {
      case 'list_plans':
        return Promise.resolve([PLAN]);
      case 'plan_items':
        return Promise.resolve(cues);
      case 'list_templates':
        return Promise.resolve([{ id: 1, name: 'Classic Serif' }]);
      case 'import_media': {
        nextMedia += 1;
        const asset = { id: nextMedia, kind: args.kind, filename: args.filename, path: `media/${nextMedia}` };
        imported.push(asset);
        return Promise.resolve(asset);
      }
      case 'add_plan_item': {
        nextCue += 1;
        cues = [
          ...cues,
          { ...planCue(nextCue), cue_type: args.cueType, label: args.label, payload_json: args.payloadJson },
        ];
        return Promise.resolve(nextCue);
      }
      case 'reorder_plan': {
        reorders.push(args.ids);
        const by = new Map(cues.map((c) => [c.id, c]));
        cues = args.ids.map((id) => by.get(id));
        return Promise.resolve(null);
      }
      case 'get_rehearsal':
        return Promise.resolve(false);
      case 'get_sensitivity':
        return Promise.resolve(50);
      default:
        return Promise.resolve([]);
    }
  });
  cap.capture.update((c) => ({ ...c, available: true }));
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

async function openPlan() {
  const ServicePlanner = (await import('./views/ServicePlanner.svelte')).default;
  app = new ServicePlanner({ target: host });
  await until(() => host.querySelector('.sp-railcard'), 'the plan rail');
  host.querySelector('.sp-railcard').click();
  await until(() => host.querySelectorAll('.sp-row').length === 3, 'the running order');
  // `onMount` opens the most recent plan itself, so let both `open()` calls
  // finish before asserting against a selection somebody else is still writing.
  await until(() => host.querySelector('.sp-row.sel'), 'the first cue selected');
  await settle(80);
}

/** A DragEvent jsdom will carry a file list on. */
function fileDrag(type, files, clientY = 0) {
  const e = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperty(e, 'dataTransfer', {
    value: { files, items: files, types: ['Files'], dropEffect: 'none' },
  });
  Object.defineProperty(e, 'clientY', { value: clientY });
  return e;
}

describe('a real drop on a mounted Planner', () => {
  itMounted('dragging a file over the list marks where it will land', async () => {
    await openPlan();
    const list = host.querySelector('.sp-tablewrap');
    list.dispatchEvent(fileDrag('dragenter', [fakeFile('sunrise.jpg')]));
    const over = fileDrag('dragover', [fakeFile('sunrise.jpg')], 400);
    list.dispatchEvent(over);
    await tick();
    // The webview would navigate to the file without this.
    expect(over.defaultPrevented).toBe(true);
    // And the operator can SEE it, in words as well as a rule.
    const mark = host.querySelector('.sp-mark');
    expect(mark).not.toBeNull();
    expect(mark.textContent).toContain('Drop here');
    expect(list.classList.contains('dropping')).toBe(true);
  });

  itMounted('leaving the list takes the marker away again', async () => {
    await openPlan();
    const list = host.querySelector('.sp-tablewrap');
    list.dispatchEvent(fileDrag('dragenter', [fakeFile('a.jpg')]));
    list.dispatchEvent(fileDrag('dragover', [fakeFile('a.jpg')], 400));
    await tick();
    expect(host.querySelector('.sp-mark')).not.toBeNull();
    list.dispatchEvent(new Event('dragleave', { bubbles: true }));
    await tick();
    expect(host.querySelector('.sp-mark')).toBeNull();
  });

  itMounted('dropping a picture imports it and adds a media cue', async () => {
    await openPlan();
    const list = host.querySelector('.sp-tablewrap');
    list.dispatchEvent(fileDrag('dragover', [fakeFile('sunrise.jpg')], 400));
    await tick();
    list.dispatchEvent(fileDrag('drop', [fakeFile('sunrise.jpg')], 400));
    await until(() => host.querySelectorAll('.sp-row').length === 4, 'the new cue');
    await settle(80);

    expect(imported).toHaveLength(1);
    expect(imported[0].kind).toBe('image');
    expect(imported[0].filename).toBe('sunrise.jpg');
    // The cue carries the asset, so the inspector paints the picture rather than
    // printing the filename.
    const added = cues.find((c) => c.cue_type === 'media');
    expect(JSON.parse(added.payload_json)).toMatchObject({
      media_id: imported[0].id,
      kind: 'image',
      filename: 'sunrise.jpg',
    });
    expect(host.querySelector('.sp-msg')?.textContent).toContain('Added 1 media cue');
  });

  itMounted('…and it lands where the marker was, not at the end of the plan', async () => {
    // THE WHOLE CLAIM OF THE MARKER: what it shows is what you get. jsdom reports
    // every rect as zero, so real geometry is stubbed onto the rows — otherwise
    // this test could only ever exercise the degenerate "append" case, which is
    // the one case where the marker and a plain add agree anyway, and it would
    // pass over a view that computed the two numbers separately.
    await openPlan();
    const list = host.querySelector('.sp-tablewrap');
    const layOut = () => {
      [...host.querySelectorAll('.sp-row')].forEach((row, i) => {
        row.getBoundingClientRect = () => ({ top: i * 30, bottom: i * 30 + 30, height: 30 });
      });
    };
    layOut();

    // y = 40 is inside row 1 (30…60) but above its midpoint (45), so the gap is 1
    // — between Cue 1 and Cue 2.
    list.dispatchEvent(fileDrag('dragover', [fakeFile('a.jpg')], 40));
    await tick();
    const kids = [...host.querySelectorAll('.sp-tablewrap > *')];
    const markedGap = kids.findIndex((el) => el.classList.contains('sp-mark'));
    expect(markedGap, 'the marker is not between the first and second rows').toBe(1);

    list.dispatchEvent(fileDrag('drop', [fakeFile('a.jpg')], 40));
    await until(() => host.querySelectorAll('.sp-row').length === 4, 'the new cue');
    await settle(80);
    const order = [...host.querySelectorAll('.sp-row .sp-cuetitle')].map((e) => e.textContent.trim());
    // Same number, both times. The reorder that was sent says so too.
    expect(order).toEqual(['Cue 1', 'a.jpg', 'Cue 2', 'Cue 3']);
    expect(reorders).toHaveLength(1);
  });

  itMounted('dropping above every row puts the cue first', async () => {
    await openPlan();
    const list = host.querySelector('.sp-tablewrap');
    [...host.querySelectorAll('.sp-row')].forEach((row, i) => {
      row.getBoundingClientRect = () => ({ top: i * 30, bottom: i * 30 + 30, height: 30 });
    });
    list.dispatchEvent(fileDrag('dragover', [fakeFile('first.jpg')], 2));
    await tick();
    list.dispatchEvent(fileDrag('drop', [fakeFile('first.jpg')], 2));
    await until(() => host.querySelectorAll('.sp-row').length === 4, 'the new cue');
    await settle(80);
    const order = [...host.querySelectorAll('.sp-row .sp-cuetitle')].map((e) => e.textContent.trim());
    expect(order[0]).toBe('first.jpg');
  });

  itMounted('dropping a PDF adds NOTHING, and says why', async () => {
    // The Sunday failure this refusal exists to stop. Watched red by letting
    // `document` through `triageDrop`.
    await openPlan();
    const list = host.querySelector('.sp-tablewrap');
    list.dispatchEvent(fileDrag('drop', [fakeFile('notices.pdf')], 400));
    await until(() => host.querySelector('.sp-err'), 'the refusal');
    expect(imported).toEqual([]);
    expect(host.querySelectorAll('.sp-row')).toHaveLength(3);
    const said = host.querySelector('.sp-err').textContent;
    expect(said).toContain('notices.pdf');
    expect(said).toContain('document');
  });

  itMounted('a mixed drop adds the picture AND reports the file it turned away', async () => {
    await openPlan();
    const list = host.querySelector('.sp-tablewrap');
    list.dispatchEvent(
      fileDrag('drop', [fakeFile('one.jpg'), fakeFile('notices.pdf')], 400),
    );
    await until(() => host.querySelectorAll('.sp-row').length === 4, 'the picture');
    await until(() => host.querySelector('.sp-err'), 'the refusal');
    expect(imported.map((a) => a.filename)).toEqual(['one.jpg']);
    expect(host.querySelector('.sp-err').textContent).toContain('notices.pdf');
  });

  itMounted('a file too large is refused in a sentence, not a raw error', async () => {
    // `humanError` prints a refusal verbatim; a volunteer must never be shown a
    // raw Rust `Err` string, and must never be shown nothing at all.
    await openPlan();
    const huge = fakeFile('service-4k.mp4');
    Object.defineProperty(huge, 'size', { value: MAX_IMPORT_BYTES + 1 });
    host.querySelector('.sp-tablewrap').dispatchEvent(fileDrag('drop', [huge], 400));
    await until(() => host.querySelector('.sp-err'), 'the refusal');
    const said = host.querySelector('.sp-err').textContent;
    expect(said).toContain('service-4k.mp4');
    expect(said).toContain('256 MB');
    expect(imported).toEqual([]);
    expect(host.querySelectorAll('.sp-row')).toHaveLength(3);
  });

  itMounted('a drag that carries no files leaves the list alone', async () => {
    // A row being dragged inside the app, or a selection of text. No marker, and
    // the event is NOT swallowed.
    await openPlan();
    const list = host.querySelector('.sp-tablewrap');
    const e = new Event('dragover', { bubbles: true, cancelable: true });
    Object.defineProperty(e, 'dataTransfer', { value: { files: [], items: [], types: ['text/plain'] } });
    list.dispatchEvent(e);
    await tick();
    expect(host.querySelector('.sp-mark')).toBeNull();
    expect(e.defaultPrevented).toBe(false);
  });

  itMounted('an empty plan says it is a drop target', async () => {
    cues = [];
    const ServicePlanner = (await import('./views/ServicePlanner.svelte')).default;
    app = new ServicePlanner({ target: host });
    await until(() => host.querySelector('.sp-drop'), 'the empty-plan placeholder');
    expect(host.querySelector('.sp-drop').textContent).toMatch(/drop a picture or a video/i);
  });
});

// ── THE HALF THAT LIVES OUTSIDE THE WEBVIEW ─────────────────────────────────
//
// Everything above proves the page handles a drop correctly. None of it proves
// the page is ever GIVEN one, and in the packaged app it would not have been.
//
// Tauri owns the window before the webview does. Its native drag-drop handler is
// ON by default, and while it is on it swallows the OS drag: the webview never
// sees `dragover`, never sees `drop`, and `dataTransfer.files` never arrives. The
// page would have looked exactly like this file says it behaves, and done
// nothing, on a Sunday, in a packaged build — which is rule 17's shape in a
// different costume: invisible in `tauri dev` reasoning and invisible in jsdom,
// visible only in the one build handed to a church.
//
// The switch is `app.windows[].dragDropEnabled`, renamed from v1's
// `fileDropEnabled`. `false` stands the native handler down and lets HTML5 drag
// and drop reach the page.
//
// So the feature is two doors, and this is the second one. A future tidy-up of
// `tauri.conf.json` that removes a key "nothing reads" takes the drop with it,
// silently, and no other test in this repository would notice.
describe('the window actually hands the drop to the page', () => {
  it('the native drag-drop handler is stood down for the main window', () => {
    const conf = JSON.parse(
      readFileSync(resolve(__dirname, '../../src-tauri/tauri.conf.json'), 'utf8'),
    );
    const windows = conf?.app?.windows ?? [];
    expect(windows.length, 'the main window is gone from the config').toBeGreaterThan(0);
    expect(
      windows[0].dragDropEnabled,
      'Tauri would swallow the OS drag and `dataTransfer.files` would never reach the Planner',
    ).toBe(false);
  });
});
