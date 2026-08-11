// Test bootstrap. Runs once per test file, before any test in it.
//
// WHY THIS FILE EXISTS: `npm test` failed on Node 22 and newer with
// `TypeError: Cannot read properties of undefined (reading 'clear')` on the very
// first `localStorage.clear()` — while passing on Node 20, which is what CI runs.
// A contributor on a current Node saw 60 red tests on a clean checkout and
// nothing in the repo explained it. Relay stores the operator's session, the
// crash record and the first-run flag in Web Storage, so those 60 tests are
// exactly the ones covering what an operator sees when the app relaunches.
//
// Node >= 22 ships Web Storage and defines `localStorage` / `sessionStorage` on
// the global as OWN accessor properties. Without `--localstorage-file` that
// accessor returns `undefined` and only warns. Vitest's jsdom environment copies
// window properties onto the global but leaves keys the global already owns — so
// Node's dead stub wins over jsdom's real Storage and bare `localStorage` in a
// test is `undefined` rather than working, or even throwing somewhere legible.
//
// Fix: hand the global a real jsdom Storage whenever the ambient one is missing.
// One private JSDOM per test file, so files stay isolated exactly as before.
// Version-independent — on Node 20 the ambient Storage is fine and this no-ops.
import { JSDOM } from 'jsdom';

const KEYS = ['localStorage', 'sessionStorage'];

if (KEYS.some((k) => !globalThis[k])) {
  // An opaque origin (the default `about:blank`) has no Web Storage at all, so
  // the URL is not decoration — without it these read back as null.
  const host = new JSDOM('', { url: 'http://localhost' }).window;
  for (const key of KEYS) {
    if (globalThis[key]) continue;
    Object.defineProperty(globalThis, key, {
      value: host[key],
      configurable: true,
      writable: true,
    });
  }
}
