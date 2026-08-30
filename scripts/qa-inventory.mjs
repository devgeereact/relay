#!/usr/bin/env node
// THE SURFACE INVENTORY — what an auditor would see if it could see.
//
// This machine cannot open Relay's window, cannot screenshot it, and cannot press
// any of its ~334 buttons. So the honest substitute for "walk every screen" is to
// derive the surface from the source and print it: every control, where it lives,
// whether anything renders it, what it dispatches, and — the question that matters
// most — whether each table in the schema can be filled by a person or only by the
// seeder.
//
// This is a REPORT, not a gate. It is regex-and-import-graph static analysis over
// Svelte templates, so it is confident about structure and heuristic about
// intent: it will occasionally miss a handler that dispatches through a variable,
// and it cannot tell a well-labelled button from a well-placed one. Every number it
// prints is a starting point for a human or an agent, never a verdict.
//
//   node scripts/qa-inventory.mjs            # markdown report
//   node scripts/qa-inventory.mjs --json     # the same data, machine-readable
//   node scripts/qa-inventory.mjs --controls # just the control table
//
// See docs/Working-Agent.md (layer C) and docs/Working-Agent-COVERAGE.md.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { resolve, dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(resolve(ROOT, p), 'utf8');
const rel = (p) => relative(ROOT, p).split('\\').join('/');

/** Every file under a directory, recursively, matching a predicate. */
function walk(dir, keep, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name.startsWith('.')) continue;
    const full = join(dir, name);
    if (statSync(full).isDirectory()) walk(full, keep, out);
    else if (keep(name)) out.push(full);
  }
  return out;
}

const lineOf = (src, index) => src.slice(0, index).split('\n').length;

// ─────────────────────────────────────────────────────────────────────────────
// 1 · REACHABILITY — is anything actually rendering this component?
//
// A finished-looking screen that nothing imports is the purest form of the defect
// this whole exercise is about: it exists, it would work, and no user can get to it.
// ─────────────────────────────────────────────────────────────────────────────

const ENTRIES = ['src/main.js', 'src/output.js', 'src/stage.js'];

function importsFrom(file, src) {
  const out = [];
  for (const m of src.matchAll(/(?:^|\n)\s*import\s+[^;]*?from\s*['"](\.[^'"]+)['"]/g)) {
    out.push(m[1]);
  }
  // Lazy routes and dynamic panels.
  for (const m of src.matchAll(/import\(\s*['"](\.[^'"]+)['"]\s*\)/g)) out.push(m[1]);
  return out.map((spec) => resolve(dirname(file), spec));
}

function reachableSet() {
  const seen = new Set();
  const queue = ENTRIES.map((e) => resolve(ROOT, e));
  while (queue.length) {
    const file = queue.pop();
    const candidates = [file, `${file}.js`, `${file}.svelte`, join(file, 'index.js')];
    const found = candidates.find((c) => {
      try {
        return statSync(c).isFile();
      } catch {
        return false;
      }
    });
    if (!found || seen.has(found)) continue;
    seen.add(found);
    let src;
    try {
      src = readFileSync(found, 'utf8');
    } catch {
      continue;
    }
    queue.push(...importsFrom(found, src));
  }
  return seen;
}

// ─────────────────────────────────────────────────────────────────────────────
// 2 · THE COMMAND MAP — the store wrapper is the only legitimate door to Rust.
// ─────────────────────────────────────────────────────────────────────────────

const captureJs = read('src/lib/stores/capture.js');
const probesJs = read('src/lib/boot/probes.js');
const mainRs = read('src-tauri/src/main.rs');

/** Registered `#[tauri::command]`s, from the one list that actually wires them up. */
function registeredCommands() {
  const block = mainRs.match(/generate_handler!\[([\s\S]*?)\]/);
  if (!block) throw new Error('could not find generate_handler! in main.rs');
  return new Set(
    block[1]
      .split(',')
      .map((s) => s.trim())
      .filter((s) => /^[a-z0-9_]+$/.test(s)),
  );
}

/**
 * Exported wrapper in capture.js → the command strings its body calls.
 *
 * Bodies are sliced from one `export function` to the next, which is crude and
 * correct often enough for an inventory: the file is written one wrapper per
 * exported function by convention.
 */
function wrapperCommandMap() {
  const map = new Map();
  const calls = new Map();
  const heads = [
    ...captureJs.matchAll(/export\s+(?:async\s+)?function\s+([A-Za-z0-9_]+)\s*\(/g),
  ];
  const names = heads.map((h) => h[1]);
  heads.forEach((h, i) => {
    const start = h.index;
    const end = i + 1 < heads.length ? heads[i + 1].index : captureJs.length;
    const body = captureJs.slice(start, end);
    const cmds = [...body.matchAll(/\bcall\(\s*['"]([a-z0-9_]+)['"]/g)].map((m) => m[1]);
    map.set(h[1], [...new Set(cmds)]);
    // Which OTHER wrappers this one calls. `startService` is only ever reached
    // from inside `beginService`, so a UI-reachability check that stops at the
    // store boundary reports it as unreachable and the `services` table as having
    // no create path — which is wrong, and would have sent an agent hunting a bug
    // that is not there.
    calls.set(
      h[1],
      names.filter((n) => n !== h[1] && new RegExp(`\\b${n}\\s*\\(`).test(body)),
    );
  });
  return { map, calls };
}

/** Wrappers reachable from a set of directly-imported ones, following store-internal calls. */
function closeOverCalls(seeds, calls) {
  const out = new Set(seeds);
  let grew = true;
  while (grew) {
    grew = false;
    for (const [from, tos] of calls) {
      if (!out.has(from)) continue;
      for (const to of tos) {
        if (!out.has(to)) {
          out.add(to);
          grew = true;
        }
      }
    }
  }
  return out;
}

// ─────────────────────────────────────────────────────────────────────────────
// 3 · CONTROLS — every thing a person can press, and what it reaches.
// ─────────────────────────────────────────────────────────────────────────────

const CONTROL_OPEN = /<(button|input|select|textarea|a)\b/gi;

/**
 * The attribute text of one tag, brace-aware.
 *
 * `[^>]*` cannot be used here: a Svelte handler is `on:click={() => go(x)}`, and
 * the `>` of the arrow ends the match three characters in. Every control in the app
 * then looks like it has no handler — 219 of them did, on this tool's first run,
 * which is a good demonstration of why a report is not a verdict.
 */
function attrsAt(src, openEnd) {
  let depth = 0;
  let quote = null;
  for (let i = openEnd; i < src.length; i++) {
    const ch = src[i];
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '{') depth++;
    else if (ch === '}') depth--;
    else if (ch === '>' && depth === 0) {
      return { attrs: src.slice(openEnd, i), end: i + 1 };
    }
  }
  return { attrs: '', end: openEnd };
}

/** The contents of a `{ … }` expression, from just after the brace to its match. */
function balanced(rest) {
  let depth = 1;
  for (let i = 0; i < rest.length; i++) {
    if (rest[i] === '{') depth++;
    else if (rest[i] === '}' && --depth === 0) return rest.slice(0, i).trim();
  }
  return rest.trim();
}

/**
 * The accessible name, as best a regex can judge it.
 *
 * **A BOUND ATTRIBUTE COUNTS.** `aria-label={tg.title}` is a name; only a static
 * string used to be, and that single omission produced four of the nine "controls
 * with no accessible name" this report used to list — including the microphone
 * toggle on the run surface and the Reset All Settings button, both of which have
 * carried an `aria-label` all along.
 *
 * That matters more than the four rows: **a report with false findings is a report
 * an operator learns to scroll past**, and this one exists to be believed. What a
 * regex cannot do is evaluate the expression — so a bound label is reported as
 * named, and whether it resolves to something useful is a question for a human
 * reading the component, not for this script to guess at.
 */
function labelFor(attrs, inner, ctx = {}) {
  // THE TWO NATIVE MECHANISMS COME FIRST, because they are the ones an author
  // should be reaching for. `<label for=…>` and a wrapping `<label>` are how HTML
  // names a form control; only `aria-label` used to count here, so a correctly
  // labelled textarea was reported as unnamed — which pushes an author towards
  // adding an `aria-label` that then has to be kept in step with the visible text.
  // A report that recommends the worse of two correct options is worse than no
  // report.
  const id = (attrs.match(/\bid=["']([^"']+)["']/i) ?? [])[1];
  if (id && ctx.labelledIds?.has(id)) return `label[for=${id}]`;
  if (ctx.inLabel) return 'wrapping <label>';
  const aria =
    attrs.match(/aria-label=["']([^"']+)["']/i) ?? attrs.match(/aria-label=\{([^}]+)\}/i);
  if (aria) return aria[1].trim();
  // `aria-labelledby` names the control from another element. Following the id is
  // beyond a regex; the presence of the attribute is not.
  if (/aria-labelledby=/i.test(attrs)) return 'aria-labelledby';
  const title =
    attrs.match(/title=["']([^"']+)["']/i) ?? attrs.match(/title=\{([^}]+)\}/i);
  const text = (inner ?? '').replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
  if (text) return text.slice(0, 60);
  if (title) return title[1].trim();
  return null;
}

/**
 * Blank out HTML comments, keeping every byte in place.
 *
 * The scanner reads raw source, and this repository comments in prose — at length,
 * about markup, quoting the markup. `VerseDeck.svelte` explains its keyboard rule in
 * a comment containing the word `<button>`, and the scanner dutifully reported a
 * handlerless button whose "label" was a fragment of the explanation. Replacing the
 * comment with spaces rather than deleting it keeps every subsequent line and column
 * correct, which is what makes the reported `file:line` worth clicking.
 */
function stripComments(src) {
  return src.replace(/<!--[\s\S]*?-->/g, (m) => m.replace(/[^\n]/g, ' '));
}

/**
 * Blank out the `<script>` block, keeping every byte in place.
 *
 * Controls live in the template; the script block only ever *talks* about them, and
 * this repository talks about them at length. `VerseDeck.svelte` explains its
 * keyboard rule in a JSDoc comment that says "the GRID card is a native `<button>`",
 * and the scanner reported a handlerless button whose label was a fragment of that
 * sentence — a finding about a paragraph.
 *
 * Blanked rather than removed, for the same reason as the comments: `file:line` has
 * to stay clickable.
 */
function stripScript(src) {
  // Case-insensitive, and tolerant of `</script >`. Not because this is a sanitiser
  // — it is a scanner over this repository's own Svelte files — but because a
  // matcher that silently ignores a casing or a stray space is one that will
  // eventually read a script block as markup, which is the exact bug `stripScript`
  // exists to prevent. CodeQL raised both, in that order, and was right twice.
  return src.replace(/<script[\s\S]*?<\/script\s*>/gi, (m) => m.replace(/[^\n]/g, ' '));
}

function controlsIn(file, rawSrc, wrappers) {
  // Two views of one file: `script` is where handlers are resolved, `src` is the
  // TEMPLATE with the script and the comments blanked out, which is the only place
  // a control can actually be.
  const script = (rawSrc.match(/<script[^>]*>([\s\S]*?)<\/script\s*>/i) ?? [, ''])[1];
  const src = stripScript(stripComments(rawSrc));
  // Which store wrappers this component imported — the vocabulary its handlers
  // can possibly speak.
  const imported = new Set();
  for (const m of script.matchAll(
    /import\s*\{([^}]+)\}\s*from\s*['"][^'"]*stores\/capture\.js['"]/g,
  )) {
    for (const name of m[1].split(',')) imported.add(name.trim().split(/\s+as\s+/)[0]);
  }

  // Every id a `<label for=…>` in this file points at.
  const labelledIds = new Set(
    [...src.matchAll(/<label[^>]*\bfor=["']([^"']+)["']/gi)].map((m) => m[1]),
  );

  const out = [];
  for (const m of src.matchAll(CONTROL_OPEN)) {
    const tag = m[1].toLowerCase();
    const { attrs, end } = attrsAt(src, m.index + m[0].length);
    if (tag === 'a' && !/href=|on:click/i.test(attrs)) continue;
    const after = src.slice(end);
    const close = after.indexOf(`</${tag}`);
    const inner = close === -1 ? '' : after.slice(0, close);

    const before = src.slice(0, m.index);
    const inLabel = before.lastIndexOf('<label') > before.lastIndexOf('</label>');

    const handler = attrs.match(/on:(?:click|change|input|submit)(?:\|\w+)*=\{([\s\S]*)/i);
    const handlerExpr = handler ? balanced(handler[1]) : null;

    // Which imported wrapper does this handler name, and which command does that
    // wrapper reach? Two hops, both textual.
    const reached = new Set();
    if (handlerExpr) {
      for (const name of imported) {
        if (new RegExp(`\\b${name}\\b`).test(handlerExpr)) {
          for (const c of wrappers.get(name) ?? []) reached.add(c);
        }
      }
      // A named local handler: look its body up in the script block.
      const named = handlerExpr.match(/^([A-Za-z0-9_$]+)$/);
      if (named) {
        const fn = script.match(
          new RegExp(`(?:async\\s+)?function\\s+${named[1]}\\s*\\([\\s\\S]{0,1200}`),
        );
        if (fn) {
          for (const name of imported) {
            if (new RegExp(`\\b${name}\\s*\\(`).test(fn[0])) {
              for (const c of wrappers.get(name) ?? []) reached.add(c);
            }
          }
        }
      }
    }

    out.push({
      file: rel(file),
      line: lineOf(src, m.index),
      tag: tag.toLowerCase(),
      label: labelFor(attrs, inner, { inLabel, labelledIds }),
      handler: handlerExpr,
      commands: [...reached],
      // `disabled` with no value is permanent; `disabled={…}` is conditional. The
      // two mean different things to this report and used to collapse to one.
      disabled: /\bdisabled\s*=\s*\{/.test(attrs)
        ? (attrs.match(/disabled=\{([^}]*)\}/) ?? [, 'true'])[1]
        : /\bdisabled(?![\w-])/.test(attrs)
          ? 'always'
          : null,
      type: (attrs.match(/type=["']([^"']+)["']/) ?? [])[1] ?? null,
      // Is this control inside a `<form>`? A submit button's handler lives there.
      inForm: before.lastIndexOf('<form') > before.lastIndexOf('</form>'),
    });
  }
  return out;
}

// ─────────────────────────────────────────────────────────────────────────────
// 4 · CREATE PATHS — can a person fill this table, or only the seeder?
//
// This is the seed-data question, restated for a product whose seed is legitimate
// content. The chain is: INSERT → db fn → #[tauri::command] → capture wrapper →
// a component that imports it. It is reported at the link where it breaks.
// ─────────────────────────────────────────────────────────────────────────────

/** Tables the app writes at RUNTIME as a consequence of operating, not of creating. */
const RUNTIME_TABLES = new Set(['services', 'transcripts', 'detections', 'cues']);
/** Tables a fresh install ships filled, on purpose. */
const SEEDED_TABLES = new Set(['verses', 'translations', 'templates', 'output_channels']);

function schemaTables() {
  const sql = read('docs/data/schema.sql');
  return [...sql.matchAll(/CREATE TABLE(?:\s+IF NOT EXISTS)?\s+([a-z_]+)\s*\(/gi)].map((m) => m[1]);
}

function rustFunctionsInsertingInto(table) {
  const files = walk(resolve(ROOT, 'src-tauri/src'), (n) => n.endsWith('.rs'));
  const fns = new Set();
  for (const f of files) {
    const src = readFileSync(f, 'utf8');
    const re = new RegExp(`INSERT\\s+(?:OR\\s+\\w+\\s+)?INTO\\s+${table}\\b`, 'gi');
    for (const m of src.matchAll(re)) {
      // The nearest `fn name(` above the INSERT owns it.
      const before = src.slice(0, m.index);
      const heads = [...before.matchAll(/\bfn\s+([a-z0-9_]+)\s*[(<]/g)];
      if (heads.length) fns.add(heads[heads.length - 1][1]);
    }
  }
  return [...fns];
}

/** Commands whose body mentions one of these db functions. */
function commandsCalling(fnNames, registered) {
  const hits = new Set();
  const heads = [...mainRs.matchAll(/#\[tauri::command\][\s\S]{0,200}?fn\s+([a-z0-9_]+)/g)];
  heads.forEach((h, i) => {
    const start = h.index;
    const end = i + 1 < heads.length ? heads[i + 1].index : mainRs.length;
    const body = mainRs.slice(start, end);
    if (fnNames.some((fn) => new RegExp(`\\b${fn}\\s*\\(`).test(body))) {
      if (registered.has(h[1])) hits.add(h[1]);
    }
  });
  return [...hits];
}

// ─────────────────────────────────────────────────────────────────────────────
// Report
// ─────────────────────────────────────────────────────────────────────────────

function build() {
  const registered = registeredCommands();
  const { map: wrappers, calls: wrapperCalls } = wrapperCommandMap();
  const reachable = reachableSet();

  const svelteFiles = walk(resolve(ROOT, 'src'), (n) => n.endsWith('.svelte'));
  const controls = [];
  const components = [];
  for (const f of svelteFiles) {
    const src = readFileSync(f, 'utf8');
    const found = controlsIn(f, src, wrappers);
    components.push({
      file: rel(f),
      rendered: reachable.has(f),
      controls: found.length,
    });
    controls.push(...found.map((c) => ({ ...c, rendered: reachable.has(f) })));
  }

  // Commands the frontend addresses.
  //
  // Deliberately looser than `ipc.test.js`'s `call('…')` shape: `clearScreens` and
  // `blackScreen` pass their command through `panicRun(cmd, label)`, so a
  // call-site-only scan reported the two most important commands in the product as
  // orphans. Any quoted occurrence of a registered name, anywhere in `src/`, counts
  // — over-broad on purpose, because a false "this is dead" is far more expensive
  // here than a false "this is used".
  const called = new Set();
  const frontendSrc = walk(resolve(ROOT, 'src'), (n) => /\.(js|svelte)$/.test(n))
    .filter((f) => !f.endsWith('.test.js'))
    .map((f) => readFileSync(f, 'utf8'))
    .join('\n');
  for (const cmd of registered) {
    if (new RegExp(`['"\`]${cmd}['"\`]`).test(frontendSrc)) called.add(cmd);
  }
  // The other direction still needs the strict shape: a name the frontend invokes
  // that Rust does not register is a live bug, and only a call site can prove it.
  const addressed = new Set();
  for (const m of captureJs.matchAll(/\bcall\(\s*['"]([a-z0-9_]+)['"]/g)) addressed.add(m[1]);
  for (const m of probesJs.matchAll(/\binvoke\(\s*['"]([a-z0-9_]+)['"]/g)) addressed.add(m[1]);

  // Wrappers a rendered component imports — plus everything those reach inside the
  // store, because a wrapper called only by another wrapper is still reachable.
  const importedDirectly = new Set();
  for (const f of [...svelteFiles, ...walk(resolve(ROOT, 'src'), (n) => n.endsWith('.js'))]) {
    if (f.endsWith('.test.js')) continue;
    if (f.endsWith('.svelte') && !reachable.has(f)) continue;
    const src = readFileSync(f, 'utf8');
    // `[^}]+`, NOT `[\s\S]*?`: a lazy match happily starts at an EARLIER import
    // statement and expands across it to reach this one, capturing both as a single
    // blob and losing every name in it. That silently turned `saveReviewedSongs`
    // into "no create path for songs".
    for (const m of src.matchAll(/import\s*\{([^}]+)\}\s*from\s*['"][^'"]*capture\.js['"]/g)) {
      for (const n of m[1].split(',')) {
        const clean = n.trim().split(/\s+as\s+/)[0];
        if (clean) importedDirectly.add(clean);
      }
    }
  }
  const importedAnywhere = closeOverCalls(importedDirectly, wrapperCalls);

  const tables = schemaTables().map((table) => {
    const fns = rustFunctionsInsertingInto(table);
    const cmds = commandsCalling(fns, registered);
    const wrapperNames = [...wrappers.entries()]
      .filter(([, cs]) => cs.some((c) => cmds.includes(c)))
      .map(([w]) => w);
    const reachedFromUi = wrapperNames.filter((w) => importedAnywhere.has(w));
    let verdict;
    if (SEEDED_TABLES.has(table) && !cmds.length) verdict = 'seeded-only';
    else if (RUNTIME_TABLES.has(table) && !cmds.length) verdict = 'runtime-only';
    else if (!fns.length) verdict = 'NO INSERT FOUND';
    else if (!cmds.length) verdict = 'BACKEND ONLY — no command reaches the insert';
    else if (!wrapperNames.length) verdict = 'COMMAND ONLY — no store wrapper calls it';
    else if (!reachedFromUi.length) verdict = 'WRAPPER ONLY — no rendered component imports it';
    else verdict = 'create path';
    return { table, fns, cmds, wrappers: wrapperNames, reachedFromUi, verdict };
  });

  return {
    generated: 'static analysis — heuristic, not a verdict',
    counts: {
      svelteFiles: svelteFiles.length,
      renderedComponents: components.filter((c) => c.rendered).length,
      orphanComponents: components.filter((c) => !c.rendered).map((c) => c.file),
      controls: controls.length,
      controlsInOrphans: controls.filter((c) => !c.rendered).length,
      registeredCommands: registered.size,
      commandsCalledByFrontend: called.size,
    },
    orphanCommands: [...registered].filter((c) => !called.has(c)).sort(),
    ghostCommands: [...addressed].filter((c) => !registered.has(c)).sort(),
    // A button with no handler, EXCEPT the two shapes where that is correct:
    //
    //  · `type="submit"` inside a form — its handler is the form's `on:submit`, and
    //    reporting it pushes an author towards a click handler that would break
    //    Enter-to-submit, which is worse than the finding.
    //  · permanently `disabled` (a bare attribute, not `disabled={…}`) — a button
    //    that is disabled by construction is a state readout, not a dead control.
    //    `ModelSetup`'s "In use" is the example, and it is right as it is.
    //
    // Both exclusions are narrow on purpose: `disabled={expr}` is still reported,
    // because a conditionally-disabled button with no handler does nothing when it
    // becomes enabled, which is exactly the bug this list is for.
    inertControls: controls
      .filter(
        (c) =>
          c.rendered &&
          c.tag === 'button' &&
          !c.handler &&
          !(c.type === 'submit' && c.inForm) &&
          c.disabled !== 'always',
      )
      .map(({ file, line, label, type }) => ({ file, line, label, type })),
    unlabelledControls: controls
      .filter((c) => c.rendered && !c.label && c.tag !== 'input')
      .map(({ file, line, tag, handler }) => ({ file, line, tag, handler })),
    tables,
    components,
    controls,
  };
}

function markdown(r) {
  const L = [];
  const c = r.counts;
  L.push('# Relay — surface inventory', '');
  L.push('_Static analysis. Structure is reliable; intent is heuristic. Not a verdict._', '');
  L.push('| | |', '|---|---|');
  L.push(`| Svelte components | ${c.svelteFiles} (${c.renderedComponents} reachable from an entry point) |`);
  L.push(`| Controls | ${c.controls} (${c.controlsInOrphans} in components nothing renders) |`);
  L.push(`| Registered commands | ${c.registeredCommands} |`);
  L.push(`| Commands the frontend addresses | ${c.commandsCalledByFrontend} |`);
  L.push('');

  L.push('## Create paths', '');
  L.push('| Table | Verdict | Command | Reached from a rendered component |');
  L.push('|---|---|---|---|');
  for (const t of r.tables) {
    L.push(
      `| \`${t.table}\` | ${t.verdict} | ${t.cmds.join(', ') || '—'} | ${t.reachedFromUi.join(', ') || '—'} |`,
    );
  }
  L.push('');

  const section = (title, rows, fmt) => {
    L.push(`## ${title} — ${rows.length}`, '');
    if (!rows.length) L.push('_none_', '');
    else {
      for (const row of rows) L.push(`- ${fmt(row)}`);
      L.push('');
    }
  };

  section('Components nothing renders', c.orphanComponents, (f) => `\`${f}\``);
  section(
    'Commands registered in Rust that no frontend caller addresses',
    r.orphanCommands,
    (x) => `\`${x}\``,
  );
  section(
    'Commands the frontend calls that Rust does not register',
    r.ghostCommands,
    (x) => `**\`${x}\`** — this one is a live bug`,
  );
  section(
    'Buttons with no handler',
    r.inertControls,
    (x) => `${x.file}:${x.line} — ${x.label ? `"${x.label}"` : '(no label)'}${x.type ? ` [type=${x.type}]` : ''}`,
  );
  section(
    'Controls with no accessible name',
    r.unlabelledControls,
    (x) => `${x.file}:${x.line} \`<${x.tag}>\` → ${x.handler ?? 'no handler'}`,
  );

  return L.join('\n');
}

const report = build();
if (process.argv.includes('--json')) {
  console.log(JSON.stringify(report, null, 2));
} else if (process.argv.includes('--controls')) {
  for (const c of report.controls) {
    console.log(
      [c.file + ':' + c.line, c.tag, c.rendered ? 'rendered' : 'ORPHAN', c.label ?? '—', c.commands.join('|') || '—'].join(
        '\t',
      ),
    );
  }
} else {
  console.log(markdown(report));
}
