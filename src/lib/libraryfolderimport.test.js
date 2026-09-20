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

// ── FINDING IT, RATHER THAN MAKING THEM FIND IT ─────────────────────────────
//
// Requirement 14: "allow the app to be able to find any propresenter files
// automatically on the computer if its ever available".
//
// A church's library is 726 files in a folder whose path the volunteer running
// Relay has very likely never seen. `Import folder` is only useful to somebody who
// already knows where it is; this is the answer for everybody else.
//
// **It finds and counts; it imports nothing.** The webview cannot open a path, so
// the operator still picks the folder — what this removes is having to know which
// one. That limitation is named on the surface rather than worked around.
describe('finding a library that is already on the computer', () => {
  itMounted('offers a scan without making the operator know where to look', () => {
    const b = [...host.querySelectorAll('button')].find((x) =>
      x.textContent.includes('Find ProPresenter'),
    );
    expect(b, 'no way to look for a library').toBeTruthy();
  });

  itMounted('reports what it found, with the path and the count', async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'find_propresenter') {
        return [{ path: '/Users/x/Documents/ProPresenter', songs: 726, truncated: false }];
      }
      return [];
    });
    const b = [...host.querySelectorAll('button')].find((x) =>
      x.textContent.includes('Find ProPresenter'),
    );
    b.click();
    await settle();
    await settle();
    expect(text()).toContain('/Users/x/Documents/ProPresenter');
    expect(text()).toContain('726 songs');
    // And it points at the door that can actually read it, because this one cannot.
    expect(text()).toContain('Import folder');
  });

  itMounted('says "at least" when the walk stopped at its bound', async () => {
    // A count that stopped early and does not say so is a wrong count — and this is
    // the number an operator checks the import against afterwards.
    invoke.mockImplementation(async (cmd) =>
      cmd === 'find_propresenter'
        ? [{ path: '/x', songs: 50_000, truncated: true }]
        : [],
    );
    [...host.querySelectorAll('button')]
      .find((x) => x.textContent.includes('Find ProPresenter'))
      .click();
    await settle();
    await settle();
    expect(text()).toContain('at least 50000 songs');
  });

  itMounted('a scan that finds nothing says what to try instead', async () => {
    // A failed scan and an empty result are the same answer here, deliberately:
    // this surface cannot tell them apart and must not pretend to. What it can do
    // is say what to do next.
    invoke.mockImplementation(async () => []);
    [...host.querySelectorAll('button')]
      .find((x) => x.textContent.includes('Find ProPresenter'))
      .click();
    await settle();
    await settle();
    expect(text()).toMatch(/No ProPresenter library in the usual places/);
    expect(text()).toContain('Import folder');
  });
});

// ── A LIBRARY'S OWN SHELVES SURVIVE THE IMPORT ──────────────────────────────
//
// A ProPresenter library is folders, and the grouping is the church's own. The
// real export: SONGS ~721, SCRIPTURES ~49, HMYN ~18, SPOKEN WORDS ~11,
// ANNOUNCEMENT ~4. Calling all 800 of them songs throws that away and makes the
// operator put it back one item at a time.
describe('a folder import keeps the shelf a file came off', () => {
  itMounted('an ANNOUNCEMENT folder lands on the announcements shelf', async () => {
    const saved = [];
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'parse_import') {
        return [{ title: 'Countdown', sections: [{ tag: '1', label: 'Slide 1', lyrics: 'Ten minutes' }] }];
      }
      if (cmd === 'save_announcement') {
        saved.push(args);
        return 1;
      }
      return [];
    });
    const f = pickedFile('Countdown.pro');
    f.webkitRelativePath = 'rrr/ANNOUNCEMENT/Countdown.pro';
    await chooseVia('input[webkitdirectory]', [f]);
    await settle();
    await settle();
    expect(saved.length, 'the announcement went to the songs shelf').toBe(1);
    expect(saved[0].title).toBe('Countdown');
    expect(saved[0].body).toContain('Ten minutes');
  });

  itMounted('a SONGS folder is untouched by any of it', async () => {
    const saved = [];
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'save_announcement') {
        saved.push(args);
        return 1;
      }
      if (cmd === 'parse_import') return [{ title: 'x', sections: [] }];
      return [];
    });
    const f = pickedFile('30 Billion.pro');
    f.webkitRelativePath = 'rrr/SONGS/30 Billion.pro';
    await chooseVia('input[webkitdirectory]', [f]);
    await settle();
    expect(saved, 'a song was filed as an announcement').toEqual([]);
  });

  itMounted('a HAND-PICKED file is a song exactly as it always was', async () => {
    // No relative path means no grouping, and guessing one from a file name would
    // file things on a shelf nobody chose. This is the regression guard on every
    // import that existed before folders did.
    const saved = [];
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'save_announcement') {
        saved.push(args);
        return 1;
      }
      if (cmd === 'parse_import') return [{ title: 'x', sections: [] }];
      return [];
    });
    await chooseVia('input[type="file"]:not([webkitdirectory])', [pickedFile('ANNOUNCEMENT.pro')]);
    await settle();
    expect(saved).toEqual([]);
  });

  itMounted('and the report names the folders in the church’s own words', async () => {
    invoke.mockImplementation(async (cmd) => (cmd === 'parse_import' ? [] : []));
    const mk = (path) => {
      const f = pickedFile(path.split('/').pop());
      f.webkitRelativePath = path;
      return f;
    };
    await chooseVia('input[webkitdirectory]', [
      mk('L/SONGS/a.pro'),
      mk('L/SONGS/b.pro'),
      mk('L/HMYN/c.pro'),
    ]);
    await settle();
    expect(text()).toContain('SONGS 2');
    expect(text()).toContain('HMYN 1');
  });
});
