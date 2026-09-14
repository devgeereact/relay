// W2 · LOOK AT IT BEFORE IT IS ADDED — `docs/REBRAND.md` §10, media uploads.
//
// "A real file, read locally into the item, previewed before it is added, with a
// caption; the slide IS the picture."
//
// Media used to be added the instant the file dialog closed. The failure that
// matters is not the import — it is `IMG_20240714_113255.jpg` sitting in a
// church's library with nothing to say which picture it is until somebody fires
// it at a congregation to find out.
//
//   npx vitest run src/lib/medialook.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import * as svelteRuntime from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture, readErrors } = await import('./stores/capture.js');

const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

let host;
let app;
const revoked = [];

/** A stand-in for a file the operator picked out of the dialog. */
function pickedFile(name, bytes = new Uint8Array([1, 2, 3])) {
  return { name, size: bytes.length, arrayBuffer: async () => bytes.buffer };
}

/** Put files on the hidden `<input type=file>` and fire its change event. */
async function choose(el, files) {
  const input = el.querySelector('input[type="file"]');
  Object.defineProperty(input, 'files', { value: files, configurable: true });
  input.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
}

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

// 150 rather than a handful: this file runs alongside seventy others and the
// machine CI uses is roughly 13x slower than this one. A flaky test is worse
// than a missing one — it trains whoever sees it red to run it again rather
// than read it (`surface.test.js` says the same thing, for the same reason).
async function until(predicate, what, tries = 150) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
  }
  throw new Error(
    `timed out waiting for: ${what}\n---- what the DOM said ----\n${host?.textContent?.slice(0, 600)}`,
  );
}

const imported = () => invoke.mock.calls.filter((c) => c[0] === 'import_media');

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
  readErrors.set({});
  capture.update((s) => ({ ...s, available: true }));
  // jsdom implements neither half of the object-URL API.
  revoked.length = 0;
  globalThis.URL.createObjectURL = (f) => `blob:relay/${f.name}`;
  globalThis.URL.revokeObjectURL = (u) => revoked.push(u);
  host = document.createElement('div');
  document.body.appendChild(host);
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  document.body.innerHTML = '';
});

async function mountLibrary() {
  const Library = (await import('./views/Library.svelte')).default;
  app = new Library({ target: host });
  await settle();
  return host;
}

describe('a picture is looked at before it is added', () => {
  itMounted('shows the file the operator chose, and imports nothing yet', async () => {
    const el = await mountLibrary();
    await choose(el, [pickedFile('IMG_20240714_113255.jpg')]);
    await until(() => el.querySelector('[role="dialog"]'), 'the media look to open');

    expect(imported(), 'nothing may reach the library before the operator says so').toEqual([]);

    const img = el.querySelector('[role="dialog"] img');
    expect(img, 'the preview is the real file, not a stand-in icon').toBeTruthy();
    expect(img.getAttribute('src')).toBe('blob:relay/IMG_20240714_113255.jpg');

    // The caption starts as the filename's stem and is editable.
    const name = el.querySelector('[role="dialog"] input.r-input');
    expect(name.value).toBe('IMG_20240714_113255');
  });

  itMounted('adds it under the name the operator typed, keeping the extension', async () => {
    const el = await mountLibrary();
    await choose(el, [pickedFile('IMG_20240714_113255.jpg')]);
    await until(() => el.querySelector('[role="dialog"]'), 'the media look to open');

    const name = el.querySelector('[role="dialog"] input.r-input');
    name.value = 'Harvest backdrop';
    name.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();

    [...el.querySelectorAll('[role="dialog"] button')]
      .find((b) => /^Add 1$/.test(b.textContent.trim()))
      .click();
    await until(() => imported().length, 'the picture to be imported');

    const [, args] = imported()[0];
    expect(args.kind).toBe('image');
    expect(args.filename).toBe('Harvest backdrop.jpg');
  });

  itMounted('cancelling adds nothing, and lets the file go', async () => {
    const el = await mountLibrary();
    await choose(el, [pickedFile('logo.png')]);
    await until(() => el.querySelector('[role="dialog"]'), 'the media look to open');

    [...el.querySelectorAll('[role="dialog"] button')]
      .find((b) => /^Cancel$/.test(b.textContent.trim()))
      .click();
    await settle();

    expect(imported()).toEqual([]);
    expect(el.querySelector('[role="dialog"]')).toBeNull();
    // An object URL per file per service is a leak in the one process that may
    // not run out of memory.
    expect(revoked).toEqual(['blob:relay/logo.png']);
  });

  itMounted('a document gets a row and a name, and no frame it cannot paint', async () => {
    const el = await mountLibrary();
    await choose(el, [pickedFile('order-of-service.pdf')]);
    await until(() => el.querySelector('[role="dialog"]'), 'the media look to open');

    expect(el.querySelector('[role="dialog"] img')).toBeNull();
    expect(el.querySelector('[role="dialog"] video')).toBeNull();
    expect(el.querySelector('[role="dialog"]').textContent).toMatch(/PDF/);
  });
});
