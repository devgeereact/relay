// IMPORTING A WHOLE SONG LIBRARY — the runner, and the report that is the point.
//
// The existing import path (`Library.svelte::onFiles` → `parse_import` →
// `ImportReview` → `save_reviewed_songs`) is correct and stays exactly as it is for
// the Saturday case: two or three files, read, edited, saved. It scales with the
// operator's attention, which is the right shape at three files.
//
// A real ProPresenter 7 export is 726 `.pro` files. Four things change at that size,
// and none of them is "the same thing but slower":
//
//  1. MEMORY. Every file is read to base64 in the webview before it crosses IPC,
//     and `capture.js::fileToBase64` documents the four simultaneous copies that
//     costs. Reading 726 of them up front is the operating system killing Relay on
//     a church laptop: no error, no message, nothing in any log. So this runner
//     reads ONE file, hands it across, drops it, and commits in batches.
//  2. STOPPING. A run this long must be stoppable, and stopping has to leave a
//     state a person can reason about.
//  3. REVIEWING. 726 entries is not a review, it is data entry — see the note in
//     `BulkImport.svelte` for the trade that was made and why.
//  4. WHAT CAME IN THIN. About forty of the 726 carry exactly one RTF block. A file
//     that yields one slide is either a one-line chorus or a parse that found
//     almost nothing, and once it is in the library those two look identical. This
//     is the single most valuable thing the run produces: the LIST OF FILES A HUMAN
//     STILL HAS TO LOOK AT. A silent partial import is worse than a failed one,
//     because the failed one gets noticed.
//
// DECISIONS §105 records the trade and why each half of it is shaped this way.
//
// This module is pure: no Tauri, no DOM, no stores. The three doors out (`read`,
// `parse`, `save`) are injected, which is what makes the memory ordering and the
// cancel semantics testable at all — `bulkimport.test.js`.

import { humanError } from './errors.js';

/**
 * How many lyric files make a run a BULK run.
 *
 * Below this, `ImportReview` is still the better product: the operator can fix a
 * title and a slide break before anything lands. Above it, that same screen asks
 * somebody to expand and read several hundred accordions, which nobody does — and a
 * review nobody performs is worse than no review, because the product then believes
 * the content was checked.
 *
 * 20 is a judgement, not a measurement: it is about where a scroll-and-skim stops
 * being possible in one sitting.
 */
export const BULK_THRESHOLD = 20;

/**
 * Songs per `save_reviewed_songs` call.
 *
 * Small enough that a refusal (the service lock) is discovered in the first few
 * seconds rather than at the end, and that a cancel loses nothing; large enough
 * that 726 songs is ~30 round trips and not 726.
 */
export const SAVE_BATCH = 25;

/** At or below this many slides, a song is reported as thin. */
export const THIN_SLIDES = 1;

/** A slide with words in it. The backend drops the others, so this must agree. */
const hasWords = (s) => String(s?.lyrics ?? '').trim().length > 0;

/** The shape `save_reviewed_songs` accepts, with the blank slides already gone. */
function keepable(song) {
  const title = String(song?.title ?? '').trim();
  const sections = (song?.sections ?? []).filter(hasWords);
  if (!title || !sections.length) return null;
  return { title, author: '', song_key: '', sections };
}

/** A fresh report. Every list starts empty and only ever grows. */
function blankReport(filesTotal) {
  return {
    filesTotal,
    filesDone: 0,
    filesRead: 0,
    current: '',
    songsParsed: 0,
    songsSaved: 0,
    added: [],
    replaced: [],
    /** [{ file, title, slides }] — the forty. */
    thin: [],
    /** [{ file, message }] — a file that yielded nothing readable. */
    unreadable: [],
    /** [{ title, files }] — one title, more than one file. */
    duplicates: [],
    cancelled: false,
    error: null,
    finished: false,
  };
}

/** What a progress listener is handed. Scalars only — it is called per file. */
const snapshot = (r) => ({
  filesTotal: r.filesTotal,
  filesDone: r.filesDone,
  current: r.current,
  songsParsed: r.songsParsed,
  songsSaved: r.songsSaved,
});

/**
 * Import many lyric files, one at a time, committing in batches.
 *
 * @param files      the picked files; only `.name` is read here, `read` does the rest
 * @param read       (file) -> base64. One file's bytes exist at a time.
 * @param parse      (name, base64) -> [{ title, sections }]. Throws for an unreadable file.
 * @param save       (songs) -> { added, replaced }. Throws when the service lock refuses.
 * @param batchSize  songs per commit
 * @param onProgress called before each file and after each commit
 * @param shouldCancel checked once per file, at the file boundary
 * @returns the report. It never throws: an abort is `report.error`.
 */
export async function runBulkImport(
  files,
  { read, parse, save },
  { batchSize = SAVE_BATCH, onProgress = () => {}, shouldCancel = () => false } = {},
) {
  const report = blankReport(files.length);
  // title (trimmed, lowercased) -> { title, files } — the same key the backend
  // dedupes on, so what this reports is what actually happened in the database.
  const byTitle = new Map();
  let pending = [];

  /**
   * Commit the pending batch.
   *
   * A save failure is NOT a per-file failure. `save_reviewed_songs` is on the
   * service lock's protected list, so the first refusal is the answer for every
   * batch after it; going on to read the other 700 files to fail 140 more times
   * wastes twenty minutes and tells the operator nothing new.
   *
   * @returns true if the run may continue.
   */
  async function flush() {
    if (!pending.length) return true;
    const batch = pending;
    pending = [];
    try {
      const res = (await save(batch)) ?? {};
      report.added.push(...(res.added ?? []));
      report.replaced.push(...(res.replaced ?? []));
      report.songsSaved += batch.length;
      onProgress(snapshot(report));
      return true;
    } catch (e) {
      report.error = humanError(e);
      return false;
    }
  }

  for (const file of files) {
    if (shouldCancel()) {
      report.cancelled = true;
      break;
    }
    report.current = file?.name ?? '';
    onProgress(snapshot(report));

    let got = null;
    try {
      const b64 = await read(file);
      got = await parse(report.current, b64);
      // `b64` goes out of scope here and nothing else holds it. That is the whole
      // memory strategy, and `bulkimport.test.js` pins it as an ordering.
    } catch (e) {
      report.unreadable.push({ file: report.current, message: humanError(e) });
      report.filesDone += 1;
      continue;
    }

    const keep = (got ?? []).map(keepable).filter(Boolean);
    if (!keep.length) {
      // The backend silently drops a song with no words in it, so a file that
      // parsed into nothing but blanks would otherwise vanish between the count
      // the operator saw and the count the library holds.
      report.unreadable.push({ file: report.current, message: 'no lyrics found in this file' });
      report.filesDone += 1;
      continue;
    }

    for (const s of keep) {
      const key = s.title.trim().toLowerCase();
      const seen = byTitle.get(key);
      if (seen) seen.files.push(report.current);
      else byTitle.set(key, { title: s.title, files: [report.current] });
      if (s.sections.length <= THIN_SLIDES) {
        report.thin.push({ file: report.current, title: s.title, slides: s.sections.length });
      }
    }

    pending.push(...keep);
    report.songsParsed += keep.length;
    report.filesRead += 1;
    report.filesDone += 1;

    if (pending.length >= batchSize && !(await flush())) {
      // Aborted. Do NOT flush again on the way out — the refusal applies to this
      // batch too, and retrying it would only produce a second copy of the error.
      pending = [];
      report.current = '';
      onProgress(snapshot(report));
      return finish(report, byTitle);
    }
  }

  if (!(await flush())) {
    pending = [];
    report.current = '';
    onProgress(snapshot(report));
    return finish(report, byTitle);
  }

  report.finished = !report.cancelled;
  report.current = '';
  onProgress(snapshot(report));
  return finish(report, byTitle);
}

/** Close the report: the duplicate register is only knowable at the end. */
function finish(report, byTitle) {
  for (const entry of byTitle.values()) {
    if (entry.files.length > 1) report.duplicates.push(entry);
  }
  return report;
}

/** "n things", with the singular right. */
const count = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/**
 * The one sentence the operator reads.
 *
 * Three outcomes, and they may never read the same (CLAUDE.md rule 35): finished,
 * stopped by the operator, and stopped by a failure. A run that was cut short is
 * never described as an import that worked.
 */
export function describeRun(report) {
  if (!report) return 'Nothing was imported.';
  const dedupe = [];
  if (report.added.length) dedupe.push(`${report.added.length} new`);
  if (report.replaced.length) dedupe.push(`${report.replaced.length} replaced`);
  const saved = report.songsSaved
    ? `${count(report.songsSaved, 'song')} saved${dedupe.length ? ` — ${dedupe.join(', ')}` : ''}.`
    : 'No songs were saved.';

  if (report.error) {
    return `${report.error} ${saved}`;
  }
  if (report.cancelled) {
    return `Stopped after ${report.filesDone} of ${count(report.filesTotal, 'file')}. ${saved}`;
  }
  return `Read ${count(report.filesDone, 'file')}. ${saved}`;
}
