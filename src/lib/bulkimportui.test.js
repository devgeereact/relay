// THE SCREEN A VOLUNTEER WATCHES FOR TWENTY MINUTES.
//
// `bulkimport.test.js` holds the runner: memory, batching, cancel, the abort. This
// file holds the only thing the operator ever sees of it, and the two claims that
// are the POINT of the feature rather than plumbing for it:
//
//   · while it runs, WHICH FILE it is on and how far through — a bar with no
//     filename under it is a spinner with extra steps;
//   · when it ends, WHICH FILES A HUMAN STILL HAS TO LOOK AT. About forty of the
//     726 in the measured export carry a single RTF block, and once they are in
//     the library a one-line chorus and a file that barely parsed are the same
//     thing. If this panel does not name them, nobody ever finds them.
//
// It drives the REAL wrappers in `capture.js` over a mocked `invoke`, so the
// command strings, the base64 hop and `errors.js` are all exercised — the mock is
// the bridge, not the import path.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const BulkImport = (await import('./views/library/BulkImport.svelte')).default;

/** A picked file. `fileToBase64` reads `size` then `arrayBuffer()`. */
const picked = (name) => ({
  name,
  size: 4,
  arrayBuffer: async () => new Uint8Array([104, 105, 33, 33]).buffer,
});

const many = (n, prefix = 'SONG') =>
  Array.from({ length: n }, (_, i) => picked(`${prefix}-${i + 1}.pro`));

const song = (title, slides = 4) => ({
  title,
  sections: Array.from({ length: slides }, (_, i) => ({
    tag: String(i + 1),
    label: `Slide ${i + 1}`,
    lyrics: `line ${i + 1}`,
  })),
});

/** A gate the test can hold a parse on, so a cancel can be timed exactly. */
let holdParse = null;
let releaseParse = null;
function hold() {
  holdParse = new Promise((r) => {
    releaseParse = () => {
      holdParse = null;
      r();
    };
  });
}

let host;
let app;

function mount(props) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new BulkImport({ target: host, props });
  return host;
}

/** capture.js reaches the bridge through a dynamic import, so tick() alone is not enough. */
async function settle(turns = 60) {
  for (let i = 0; i < turns; i += 1) {
    await Promise.resolve();
    await tick();
  }
}

const text = () => host.textContent.replace(/\s+/g, ' ');
const btn = (label) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim().includes(label));

beforeEach(() => {
  invoke.mockReset();
  holdParse = null;
  releaseParse = null;
  invoke.mockImplementation(async (cmd, args) => {
    if (cmd === 'parse_import') {
      if (holdParse) await holdParse;
      return [song(args.filename.replace(/\.pro$/, ''))];
    }
    if (cmd === 'save_reviewed_songs') {
      return { added: args.songs.map((s) => s.title), replaced: [] };
    }
    return null;
  });
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
});

describe('while it is running', () => {
  it('names the file it is on and how far through it is', async () => {
    hold();
    mount({ files: many(726) });
    await settle(10);
    expect(text()).toContain('SONG-1.pro');
    expect(text()).toContain('of 726');
    releaseParse();
  });

  it('offers a progressbar a screen reader can read, not a bare animation', async () => {
    hold();
    mount({ files: many(50) });
    await settle(10);
    const bar = host.querySelector('[role="progressbar"]');
    expect(bar).toBeTruthy();
    expect(bar.getAttribute('aria-valuemax')).toBe('50');
    expect(bar.getAttribute('aria-label')).toBeTruthy();
    releaseParse();
  });

  it('can be stopped, and does not call that a finished import', async () => {
    hold();
    mount({ files: many(50) });
    await settle(10);
    const stop = btn('Stop');
    expect(stop).toBeTruthy();
    stop.click();
    releaseParse();
    await settle();
    expect(text()).toMatch(/Stopped after/);
    expect(text()).not.toMatch(/Read 50 files/);
  });

  it('keeps what it had already read when it is stopped', async () => {
    hold();
    mount({ files: many(50) });
    await settle(10);
    btn('Stop').click();
    releaseParse();
    await settle();
    // One file was in flight when Stop was pressed; its song is committed rather
    // than silently dropped, so the number on screen is the number in the library.
    expect(invoke).toHaveBeenCalledWith('save_reviewed_songs', expect.anything());
    expect(text()).toMatch(/1 song saved/);
  });
});

describe('the report — the reason this feature exists', () => {
  it('names every file that came in with a single slide', async () => {
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'parse_import') {
        const thin = args.filename === 'SONG-2.pro';
        return [song(args.filename.replace(/\.pro$/, ''), thin ? 1 : 5)];
      }
      return { added: args.songs.map((s) => s.title), replaced: [] };
    });
    mount({ files: many(3) });
    await settle();
    expect(text()).toContain('SONG-2.pro');
    expect(text()).toMatch(/one slide|1 slide/i);
  });

  it('says nothing about thin files when none came in thin', async () => {
    mount({ files: many(3) });
    await settle();
    expect(text()).not.toMatch(/came in thin/i);
  });

  it('names a file it could not read, having carried on past it', async () => {
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'parse_import') {
        if (args.filename === 'SONG-2.pro') {
          return Promise.reject({ kind: 'refused', message: 'no lyrics found in this file' });
        }
        return [song(args.filename)];
      }
      return { added: args.songs.map((s) => s.title), replaced: [] };
    });
    mount({ files: many(4) });
    await settle();
    expect(text()).toContain('SONG-2.pro');
    expect(text()).toContain('no lyrics found in this file');
    expect(text()).toMatch(/3 songs saved/);
  });

  it('summarises the dedupe rather than leaving the operator to infer it', async () => {
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'parse_import') return [song(args.filename.replace(/\.pro$/, ''))];
      return {
        added: args.songs.slice(0, 2).map((s) => s.title),
        replaced: args.songs.slice(2).map((s) => s.title),
      };
    });
    mount({ files: many(5) });
    await settle();
    expect(text()).toContain('2 new');
    expect(text()).toContain('3 replaced');
  });

  it('names a title that arrived from more than one file', async () => {
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'parse_import') {
        return [song(args.filename === 'SONG-3.pro' ? 'SONG-1' : args.filename.replace(/\.pro$/, ''))];
      }
      return { added: args.songs.map((s) => s.title), replaced: [] };
    });
    mount({ files: many(3) });
    await settle();
    expect(text()).toMatch(/more than one file/i);
    expect(text()).toContain('SONG-3.pro');
  });

  it('says plainly that section names did not come across, because the UI would otherwise imply they did', async () => {
    // `proimport.rs` takes the title from the file stem and `main.rs` renames every
    // slide `Slide N`. A library full of "Slide 1 … Slide 9" with no Verse/Chorus is
    // exactly what an operator would read as a bug unless they are told first.
    mount({ files: many(3) });
    await settle();
    expect(text()).toMatch(/Slide 1/);
    expect(text()).toMatch(/arrangement/i);
  });

  it('hands the report back when the operator is done with it', async () => {
    mount({ files: many(3) });
    await settle();
    const got = [];
    app.$on('done', (e) => got.push(e.detail));
    btn('Done').click();
    await tick();
    expect(got).toHaveLength(1);
    expect(got[0].songsSaved).toBe(3);
  });
});

describe('a refusal is a sentence, never an Err', () => {
  it('shows the service lock in words a volunteer can act on', async () => {
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'parse_import') return [song(args.filename)];
      return Promise.reject({
        kind: 'refused',
        message:
          'A service is being recorded, so Relay is holding this back: saving an import. ' +
          'It can wait until the service ends — or unlock in Settings → Before the service if you need to do it now.',
      });
    });
    mount({ files: many(3) });
    await settle();
    expect(text()).toContain('A service is being recorded');
    expect(text()).toContain('Settings');
    expect(text()).not.toMatch(/\[object Object\]/);
  });

  it('humanises an unrecognised failure rather than printing the Rust string bare', async () => {
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'parse_import') return [song(args.filename)];
      return Promise.reject('database is locked');
    });
    mount({ files: many(2) });
    await settle();
    expect(text()).toContain('Try once more');
  });
});

describe('rule 44 — this panel takes nothing from the operator', () => {
  it('mounts no dialog, menu or listbox, so `shortcuts.js` never stands down for it', () => {
    // A twenty-minute import may not be a twenty-minute window with no Escape and
    // no `Clear screens`. The panel is an ordinary region in the workspace.
    const src = readFileSync(join(process.cwd(), 'src/lib/views/library/BulkImport.svelte'), 'utf8');
    const code = codeOnly(src);
    for (const role of ['dialog', 'alertdialog', 'menu', 'listbox']) {
      expect(code, `a ${role} would take Escape and the panic button`).not.toContain(
        `role="${role}"`,
      );
    }
  });
});
