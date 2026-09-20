// A WHOLE LIBRARY IS A FOLDER, NOT A SELECTION.
//
// `Import` has always taken many files at once — `<input multiple>`, and twenty
// or more lyric files route to the bulk runner. What it could not take was a
// DIRECTORY, and a real ProPresenter library is 726 files inside one. Asking an
// operator to select 726 entries in a dialog is asking them not to bother.
//
// The folder door is the same handler, the same routing and the same runner. What
// it adds is the junk, because a directory pick hands over everything underneath
// it rather than a chosen list. Measured against the real archive this exists
// for: 726 `.pro` files, a `__MACOSX` tree of AppleDouble stubs named `._X.pro`
// (4 KB of resource fork carrying the same extension, parsing to nothing), a
// `.DS_Store`, and a dozen extensionless configuration files called `Library`,
// `Media`, `Stage` and `Timers`.
//
// Without the filter those stubs are read, parsed, and reported as 726 songs that
// came in empty — which is indistinguishable from a broken parser, on the one
// screen whose job is to tell the operator which files came in thin.
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
let reads;
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

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

/** Hand files to a specific input, the way a picker does. */
async function chooseVia(selector, files) {
  const input = host.querySelector(selector);
  expect(input, `no input matched ${selector}`).toBeTruthy();
  Object.defineProperty(input, 'files', { value: files, configurable: true });
  input.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
}

const text = () => host.textContent.replace(/\s+/g, ' ');

beforeEach(() => {
  invoke.mockReset();
  generation += 1;
  reads = [];
  invoke.mockImplementation(async (cmd, args) => {
    if (cmd === 'parse_import') {
      return [
        {
          title: String(args.filename).replace(/\.[^.]*$/, ''),
          sections: [{ tag: '1', label: 'Slide 1', lyrics: 'a' }],
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

describe('there is a door for a folder, beside the one for files', () => {
  itMounted('the directory input exists and asks for a directory', () => {
    const dir = host.querySelector('input[type="file"][webkitdirectory]');
    expect(dir, 'no folder picker — an operator must select 726 files by hand').toBeTruthy();
    expect(dir.multiple, 'a folder pick that takes one file is not a folder pick').toBe(true);
  });

  itMounted('and it carries no accept list, which would filter a folder rather than files', () => {
    const dir = host.querySelector('input[type="file"][webkitdirectory]');
    expect(dir.getAttribute('accept')).toBeNull();
  });

  itMounted('the file door is untouched and still takes many at once', () => {
    const file = host.querySelector('input[type="file"]:not([webkitdirectory])');
    expect(file.multiple).toBe(true);
    expect(file.getAttribute('accept')).toContain('.pro');
  });
});

describe('what a real ProPresenter folder brings with it', () => {
  itMounted('AppleDouble stubs are never read, however much they look like songs', async () => {
    // `._Blessed be the Lord .pro` carries the same extension as the song beside
    // it and is four kilobytes of resource fork. Reading one costs a parse and
    // yields nothing, and 726 of them is a report saying every file came in empty.
    const songs = Array.from({ length: BULK_THRESHOLD }, (_, i) => pickedFile(`SONG-${i}.pro`));
    const stubs = Array.from({ length: BULK_THRESHOLD }, (_, i) => pickedFile(`._SONG-${i}.pro`));
    await chooseVia('input[webkitdirectory]', [...stubs, ...songs]);
    await settle();
    await settle();
    expect(
      reads.filter((n) => n.startsWith('._')),
      'an AppleDouble stub was read as if it were a song',
    ).toEqual([]);
  });

  itMounted('a library config file with no extension is not an unsupported format', async () => {
    // `Library`, `Media`, `Stage`, `Timers` — a dozen of them, all extensionless.
    // They are not an error the operator needs to see; they are what a folder
    // contains.
    await chooseVia('input[webkitdirectory]', [
      pickedFile('Library'),
      pickedFile('Stage'),
      pickedFile('Timers'),
    ]);
    expect(text()).not.toContain('unsupported');
  });

  itMounted('a folder with nothing importable in it says so, rather than nothing', async () => {
    // Silence here would read as "it worked". The count is what tells an operator
    // they picked the wrong folder.
    await chooseVia('input[webkitdirectory]', [
      pickedFile('.DS_Store'),
      pickedFile('Library'),
      pickedFile('._x.pro'),
    ]);
    expect(text()).toContain('Nothing importable');
  });

  itMounted('and the songs underneath all of it still reach the bulk runner', async () => {
    const junk = [pickedFile('.DS_Store'), pickedFile('Media'), pickedFile('._SONG-0.pro')];
    const songs = Array.from({ length: BULK_THRESHOLD }, (_, i) => pickedFile(`SONG-${i}.pro`));
    await chooseVia('input[webkitdirectory]', [...junk, ...songs]);
    await settle();
    expect(text(), 'the folder pick did not reach the bulk panel').toMatch(/Stop|import/i);
    expect(reads.some((n) => n.startsWith('SONG-')), 'no song was read at all').toBe(true);
  });
});
