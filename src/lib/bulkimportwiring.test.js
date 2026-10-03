// WHERE THE LIBRARY DECIDES WHICH IMPORT IT IS DOING.
//
// Two products share one `Import` button, and the split has to happen BEFORE a
// single byte is read:
//
//   a handful  → `ImportReview.svelte`, unchanged. Read, edit, then save.
//   a library  → `BulkImport.svelte`. Read one file at a time, save in batches,
//                then a report naming the files a human still has to open.
//
// The line below that matters most is not which panel appears. It is that
// `onFiles` no longer reads the lyric files itself. It used to loop
// `await parseImport(file.name, await fileToBase64(file))` over everything the
// operator picked, which at 726 files builds 726 base64 strings in the webview
// before anything is decided — and `capture.js::fileToBase64` documents the four
// simultaneous copies each of those costs. That is not a slow import; it is the
// operating system killing Relay on a church laptop with nothing in any log.
//
// So the test that guards it asserts a COUNT AT A MOMENT: with the first parse
// held open, exactly one file has been read.
//
//   npx vitest run src/lib/bulkimportwiring.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import * as svelteRuntime from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const Library = (await import('./views/Library.svelte')).default;
const { capture, readErrors } = await import('./stores/capture.js');
const { BULK_THRESHOLD } = await import('./bulkimport.js');

const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

let host;
let app;
let reads; // every file whose bytes were actually pulled into the webview

// A run started by an earlier test carries on after its component is destroyed —
// nothing in the product cancels it — so a file remembers WHICH test made it and
// a stale read is never counted against the next one. Without this the assertion
// below is flaky in exactly the direction that would hide the bug it guards.
let generation = 0;

function pickedFile(name) {
  const bytes = new Uint8Array([104, 105]);
  const mine = generation;
  return {
    name,
    size: bytes.length,
    arrayBuffer: async () => {
      if (mine === generation) reads.push(name);
      return bytes.buffer;
    },
  };
}

const many = (n, ext = 'pro') =>
  Array.from({ length: n }, (_, i) => pickedFile(`SONG-${i + 1}.${ext}`));

/** A gate the test can hold the first parse on. */
let holdParse = null;
let releaseParse = () => {};
function hold() {
  holdParse = new Promise((r) => {
    releaseParse = () => {
      holdParse = null;
      r();
    };
  });
}

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

async function until(predicate, what, tries = 150) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
  }
  throw new Error(
    `timed out waiting for: ${what}\n---- what the DOM said ----\n${host?.textContent?.slice(0, 600)}`,
  );
}

async function choose(files) {
  const input = host.querySelector('input[type="file"]');
  Object.defineProperty(input, 'files', { value: files, configurable: true });
  input.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
}

const text = () => host.textContent.replace(/\s+/g, ' ');
const btn = (label) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim().includes(label));

beforeEach(() => {
  invoke.mockReset();
  generation += 1;
  reads = [];
  holdParse = null;
  invoke.mockImplementation(async (cmd, args) => {
    if (cmd === 'parse_import') {
      if (holdParse) await holdParse;
      return [
        {
          title: String(args.filename).replace(/\.[^.]*$/, ''),
          sections: [
            { tag: '1', label: 'Slide 1', lyrics: 'a' },
            { tag: '2', label: 'Slide 2', lyrics: 'b' },
          ],
        },
      ];
    }
    if (cmd === 'save_reviewed_songs') {
      return { added: args.songs.map((s) => s.title), replaced: [] };
    }
    return [];
  });
  readErrors.set({});
  capture.update((s) => ({ ...s, available: true }));
  globalThis.URL.createObjectURL = (f) => `blob:relay/${f.name}`;
  globalThis.URL.revokeObjectURL = () => {};
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Library({ target: host, props: {} });
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  document.body.innerHTML = '';
});

describe('a handful of files is the product it always was', () => {
  itMounted('still goes through the per-song review', async () => {
    await choose(many(3));
    await until(() => /Review import/.test(text()), 'the pre-save review');
    expect(invoke.mock.calls.filter((c) => c[0] === 'parse_import')).toHaveLength(3);
    expect(text()).not.toMatch(/Importing songs|Import report/);
  });
});

describe('a whole library takes the other path', () => {
  itMounted('opens the bulk panel instead of a 726-row review', async () => {
    hold();
    await choose(many(BULK_THRESHOLD));
    await until(() => /Importing songs/.test(text()), 'the bulk panel');
    expect(text()).not.toMatch(/Review import/);
    releaseParse();
  });

  itMounted('has read exactly ONE file at the moment the panel appears', async () => {
    // THE MEMORY GUARANTEE AT THE WIRING LEVEL. The old `onFiles` read every picked
    // file to base64 before deciding anything; this assertion is 1 against 40.
    hold();
    await choose(many(40));
    await until(() => /Importing songs/.test(text()), 'the bulk panel');
    expect(reads).toEqual(['SONG-1.pro']);
    releaseParse();
  });

  itMounted('reports what happened when the operator closes it', async () => {
    await choose(many(BULK_THRESHOLD));
    await until(() => /Import report/.test(text()), 'the report');
    btn('Done').click();
    await settle();
    expect(text()).toMatch(new RegExp(`${BULK_THRESHOLD} songs saved`));
  });

  itMounted('sends a refused run to the error line, not the success line', async () => {
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'parse_import') {
        return [{ title: args.filename, sections: [{ tag: '1', label: 'Slide 1', lyrics: 'a' }] }];
      }
      if (cmd === 'save_reviewed_songs') {
        return Promise.reject({
          kind: 'refused',
          message: 'A service is being recorded, so Relay is holding this back: saving an import.',
        });
      }
      return [];
    });
    await choose(many(BULK_THRESHOLD));
    await until(() => /Import report/.test(text()), 'the report');
    btn('Done').click();
    await settle();
    const alert = host.querySelector('[role="alert"]');
    expect(alert?.textContent).toMatch(/A service is being recorded/);
  });
});

describe('a mixed pick still sorts itself', () => {
  itMounted('sends the songs to the bulk panel and the pictures to the media look', async () => {
    hold();
    await choose([...many(BULK_THRESHOLD), pickedFile('banner.png')]);
    await until(() => /Importing songs/.test(text()), 'the bulk panel');
    // The picture is not read here either — `commitMedia` reads it when the
    // operator presses Add, which is unchanged.
    expect(reads).not.toContain('banner.png');
    releaseParse();
  });
});
