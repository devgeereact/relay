// 726 FILES IS A DIFFERENT PRODUCT FROM ONE FILE.
//
// The Library's import path was written for the Saturday case: a volunteer picks
// two or three lyric files, reads them, edits them, saves. Every part of it scales
// linearly with the operator's attention, which is exactly right at three files
// and is not a plan at 726.
//
// A real ProPresenter 7 export was measured: 726 `.pro` files, 14,364 RTF blocks,
// `JESUS.pro` alone carrying 332 — and about forty files carrying exactly ONE
// block. That last set is the whole reason this module exists. A file that yields
// one slide is either a one-line chorus (fine) or a parse that found almost
// nothing (not fine), and NOTHING IN THE PRODUCT COULD TELL THE OPERATOR WHICH,
// because both land in the library looking identical. A silent partial import is
// worse than a failed one: the failed one gets noticed.
//
// Three other things only appear at volume, and each has a test below:
//
//   · MEMORY. Every file is read to base64 in the webview before it crosses IPC
//     (`capture.js::fileToBase64` documents the four simultaneous copies). Holding
//     726 of those is the operating system killing Relay on a church laptop with
//     no error in any log. So the runner reads ONE file at a time and commits in
//     batches, and `reads one file at a time` below is the guard on that.
//   · CANCELLATION. A twenty-minute run must be stoppable, and stopping must leave
//     a state somebody can reason about — not half a song, not a silent rollback
//     of work already committed.
//   · THE SERVICE LOCK. `save_reviewed_songs` is protected (`servicelock.rs`), so a
//     bulk run started over a recorded service fails on its FIRST commit and will
//     fail on every one after it. That is an abort, not a per-file failure.
//
// A per-FILE failure is the opposite: `parse_import` returns a refusal ("no lyrics
// found in this file") for anything it cannot read, and at 726 files that must not
// end the run. Collect it, name the file, carry on.

import { describe, it, expect, vi } from 'vitest';

const {
  runBulkImport,
  describeRun,
  BULK_THRESHOLD,
  SAVE_BATCH,
  THIN_SLIDES,
} = await import('./bulkimport.js');

/** A stand-in for a picked file. Only `name` is read by the runner. */
const picked = (name) => ({ name, size: 1024 });

/** N files named like a real export. */
const many = (n, prefix = 'SONG') =>
  Array.from({ length: n }, (_, i) => picked(`${prefix}-${i + 1}.pro`));

/** A parsed song with `n` slides. */
const song = (title, n = 4) => ({
  title,
  sections: Array.from({ length: n }, (_, i) => ({
    tag: String(i + 1),
    label: `Slide ${i + 1}`,
    lyrics: `line ${i + 1}`,
  })),
});

/** The happy-path dependencies: every file parses to one four-slide song. */
function deps(over = {}) {
  return {
    read: vi.fn(async () => 'YmFzZTY0'),
    parse: vi.fn(async (name) => [song(name.replace(/\.pro$/, ''))]),
    save: vi.fn(async (songs) => ({
      added: songs.map((s) => s.title),
      replaced: [],
    })),
    ...over,
  };
}

describe('the thresholds are named, not sprinkled', () => {
  it('a bulk run is a run big enough that reviewing each entry is not a plan', () => {
    expect(BULK_THRESHOLD).toBeGreaterThan(1);
    expect(SAVE_BATCH).toBeGreaterThan(1);
    // "Thin" is the finding this module exists for: one slide.
    expect(THIN_SLIDES).toBe(1);
  });
});

describe('reading 726 files', () => {
  it('reads one file at a time — the next read never starts before the last parse ends', async () => {
    // THE MEMORY GUARANTEE, stated as an ORDERING. Holding 726 base64 strings is
    // the failure; a runner that reads them all up front would interleave here.
    const order = [];
    let live = 0;
    let mostLive = 0;
    const d = deps({
      read: vi.fn(async (f) => {
        live += 1;
        mostLive = Math.max(mostLive, live);
        order.push(`read:${f.name}`);
        return 'YmFzZTY0';
      }),
      parse: vi.fn(async (name) => {
        order.push(`parse:${name}`);
        live -= 1; // the base64 is released the moment it has crossed
        return [song(name)];
      }),
    });
    await runBulkImport(many(5), d);
    expect(mostLive).toBe(1);
    expect(order.slice(0, 4)).toEqual([
      'read:SONG-1.pro',
      'parse:SONG-1.pro',
      'read:SONG-2.pro',
      'parse:SONG-2.pro',
    ]);
  });

  it('commits in batches rather than in one 726-song write', async () => {
    const d = deps();
    const report = await runBulkImport(many(60), d, { batchSize: 25 });
    const sizes = d.save.mock.calls.map(([songs]) => songs.length);
    expect(sizes).toEqual([25, 25, 10]);
    expect(report.songsSaved).toBe(60);
    expect(report.finished).toBe(true);
  });

  it('sums added and replaced across every batch, so the operator is told and not left to infer', async () => {
    const d = deps({
      save: vi.fn(async (songs) => ({
        added: songs.slice(0, 1).map((s) => s.title),
        replaced: songs.slice(1).map((s) => s.title),
      })),
    });
    const report = await runBulkImport(many(10), d, { batchSize: 5 });
    expect(report.added).toHaveLength(2); // one per batch
    expect(report.replaced).toHaveLength(8);
  });

  it('reports progress by name, so the operator can see WHICH file it is on', async () => {
    const seen = [];
    await runBulkImport(many(3), deps(), {
      onProgress: (p) => seen.push(`${p.filesDone}/${p.filesTotal}:${p.current}`),
    });
    expect(seen[0]).toBe('0/3:SONG-1.pro');
    expect(seen.at(-1)).toContain('3/3');
  });
});

describe('the forty files that came in thin — the finding this module exists for', () => {
  it('names every file that yielded a single slide', async () => {
    const d = deps({
      parse: vi.fn(async (name) => [song(name, name === 'SONG-2.pro' ? 1 : 5)]),
    });
    const report = await runBulkImport(many(3), d);
    expect(report.thin).toEqual([{ file: 'SONG-2.pro', title: 'SONG-2.pro', slides: 1 }]);
    // And it is still SAVED — a one-line chorus is a real song. The report is the
    // instrument, not a refusal.
    expect(report.songsSaved).toBe(3);
  });

  it('does not call a four-slide song thin', async () => {
    const report = await runBulkImport(many(3), deps());
    expect(report.thin).toEqual([]);
  });
});

describe('a file that yields nothing must not end the run', () => {
  it('collects the refusal, names the file, and carries on', async () => {
    const d = deps({
      parse: vi.fn(async (name) => {
        if (name === 'SONG-2.pro') {
          throw { kind: 'refused', message: 'no lyrics found in this file' };
        }
        return [song(name)];
      }),
    });
    const report = await runBulkImport(many(4), d);
    expect(report.unreadable).toEqual([
      { file: 'SONG-2.pro', message: 'no lyrics found in this file' },
    ]);
    expect(report.songsSaved).toBe(3);
    expect(report.filesDone).toBe(4);
    expect(report.finished).toBe(true);
  });

  it('treats a file whose every slide is blank the same way, because the backend would drop it silently', async () => {
    const d = deps({
      parse: vi.fn(async (name) =>
        name === 'SONG-2.pro'
          ? [{ title: 'SONG-2', sections: [{ tag: '1', label: 'Slide 1', lyrics: '   ' }] }]
          : [song(name)],
      ),
    });
    const report = await runBulkImport(many(3), d);
    expect(report.unreadable.map((u) => u.file)).toEqual(['SONG-2.pro']);
    expect(d.save.mock.calls.flatMap(([s]) => s).map((s) => s.title)).not.toContain('SONG-2');
  });
});

describe('stopping a twenty-minute run', () => {
  it('stops reading further files and says how far it got', async () => {
    let n = 0;
    const report = await runBulkImport(many(20), deps(), {
      batchSize: 5,
      shouldCancel: () => ++n > 7, // cancel is checked once per file
    });
    expect(report.cancelled).toBe(true);
    expect(report.filesDone).toBeLessThan(20);
    expect(report.filesDone).toBeGreaterThan(0);
  });

  it('commits what it had already parsed, so parsed and saved agree', async () => {
    // The alternative — discarding the part-filled batch — leaves the operator
    // with "read 312, saved 300" and no way to tell which twelve went missing.
    let n = 0;
    const d = deps();
    const report = await runBulkImport(many(20), d, {
      batchSize: 25, // deliberately larger than the run, so nothing flushes early
      shouldCancel: () => ++n > 6,
    });
    expect(report.cancelled).toBe(true);
    expect(report.songsSaved).toBe(report.songsParsed);
    expect(report.songsSaved).toBeGreaterThan(0);
    expect(d.save).toHaveBeenCalledTimes(1);
  });

  it('leaves songs saved by earlier batches saved', async () => {
    let n = 0;
    const d = deps();
    const report = await runBulkImport(many(30), d, {
      batchSize: 5,
      shouldCancel: () => ++n > 12,
    });
    expect(report.added.length).toBe(report.songsSaved);
    expect(report.songsSaved).toBeGreaterThanOrEqual(10);
  });
});

describe('the service lock is an abort, not a skipped file', () => {
  const LOCKED = {
    kind: 'refused',
    message:
      'A service is being recorded, so Relay is holding this back: saving an import. ' +
      'It can wait until the service ends — or unlock in Settings → Before the service if you need to do it now.',
  };

  it('stops the whole run on the first refused commit', async () => {
    const d = deps({
      save: vi.fn(async () => {
        throw LOCKED;
      }),
    });
    const report = await runBulkImport(many(30), d, { batchSize: 5 });
    expect(report.error).toContain('A service is being recorded');
    expect(report.finished).toBe(false);
    // It did not go on reading the other 700 files to fail 140 more times.
    expect(d.save).toHaveBeenCalledTimes(1);
    expect(report.filesDone).toBeLessThan(30);
  });

  it('keeps the count of what HAD been committed before the refusal', async () => {
    let calls = 0;
    const d = deps({
      save: vi.fn(async (songs) => {
        if (++calls > 2) throw LOCKED;
        return { added: songs.map((s) => s.title), replaced: [] };
      }),
    });
    const report = await runBulkImport(many(30), d, { batchSize: 5 });
    expect(report.songsSaved).toBe(10);
    expect(report.error).toBeTruthy();
  });

  it('humanises the refusal rather than handing a volunteer a raw Err', async () => {
    const d = deps({
      save: vi.fn(async () => {
        throw 'database is locked';
      }),
    });
    const report = await runBulkImport(many(5), d, { batchSize: 2 });
    expect(report.error).toMatch(/Try once more/);
    expect(report.error).not.toMatch(/^database is locked$/);
  });
});

describe('duplicate titles at volume', () => {
  it('names a title that arrived from more than one file', async () => {
    // `save_reviewed_songs` dedupes by title, so the second file silently REPLACES
    // the first. At three files you notice. At 726 you do not.
    const d = deps({
      parse: vi.fn(async (name) =>
        name === 'SONG-3.pro' ? [song('SONG-1')] : [song(name.replace(/\.pro$/, ''))],
      ),
    });
    const report = await runBulkImport(many(3), d);
    expect(report.duplicates).toEqual([
      { title: 'SONG-1', files: ['SONG-1.pro', 'SONG-3.pro'] },
    ]);
  });

  it('matches titles the way the backend does — trimmed, ignoring case', async () => {
    const d = deps({
      parse: vi.fn(async (name) => [song(name === 'SONG-2.pro' ? ' song-1 ' : 'SONG-1')]),
    });
    const report = await runBulkImport(many(2), d);
    expect(report.duplicates).toHaveLength(1);
  });

  it('says nothing when every title is its own', async () => {
    const report = await runBulkImport(many(5), deps());
    expect(report.duplicates).toEqual([]);
  });
});

describe('the sentence the operator reads', () => {
  it('leads with the outcome and counts both halves of the dedupe', async () => {
    const d = deps({
      save: vi.fn(async (songs) => ({
        added: songs.slice(0, 2).map((s) => s.title),
        replaced: songs.slice(2).map((s) => s.title),
      })),
    });
    const said = describeRun(await runBulkImport(many(5), d, { batchSize: 5 }));
    expect(said).toContain('5 songs');
    expect(said).toContain('2 new');
    expect(said).toContain('3 replaced');
  });

  it('says it was stopped when it was stopped, and never calls that a success', async () => {
    let n = 0;
    const said = describeRun(
      await runBulkImport(many(20), deps(), { batchSize: 5, shouldCancel: () => ++n > 6 }),
    );
    expect(said).toMatch(/[Ss]topped/);
  });

  it('leads with the failure when the run aborted', async () => {
    const d = deps({
      save: vi.fn(async () => {
        throw { kind: 'refused', message: 'A service is being recorded, so Relay is holding this back.' };
      }),
    });
    const said = describeRun(await runBulkImport(many(5), d, { batchSize: 2 }));
    expect(said).toContain('A service is being recorded');
  });

  it('never returns an empty string, whatever the run did', async () => {
    expect(describeRun(await runBulkImport([], deps()))).toBeTruthy();
  });
});
