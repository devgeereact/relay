#!/usr/bin/env node
// Repair a debug recording whose RIFF size fields overflowed (RG-316).
//
// `write_wav_f32` computed both size fields as `(samples.len() * 4) as u32`, a
// truncating cast, so any recording over 4 GiB declares a wrapped length and every
// reader stops there. The two files this machine produced hold 16.25 h and 16.03 h
// and declare 2.73 h and 2.50 h. The audio is all present; only the header lies.
//
// This reads the real audio — file length, not the declared length — and writes it
// out as a sequence of ordinary WAV files, each under the 4 GiB the format allows.
// It NEVER modifies or deletes the input.
//
//   node scripts/split-recording.mjs <file.wav>              # report only
//   node scripts/split-recording.mjs <file.wav> --write       # write the segments
//   node scripts/split-recording.mjs <file.wav> --write --out DIR
//
// Why segments and not RF64: every segment here is a WAV that every transcriber,
// editor and player already opens. RF64 is the technically correct fix and much
// less software reads it, and this file exists to be handed to whoever can measure
// word error rate from it.

import {
  existsSync,
  mkdirSync,
  openSync,
  readSync,
  writeSync,
  closeSync,
  statSync,
  fstatSync,
} from 'node:fs';
import { basename, extname, join, dirname } from 'node:path';

const HEADER = 44;
const MAX_DATA = 0xffffffff - 36; // what a 32-bit size field can describe

function fail(msg) {
  console.error(`error: ${msg}`);
  process.exit(1);
}

const args = process.argv.slice(2);
const file = args.find((a) => !a.startsWith('--'));
const write = args.includes('--write');
const outIdx = args.indexOf('--out');
const outDir = outIdx >= 0 ? args[outIdx + 1] : null;

if (!file) fail('usage: split-recording.mjs <file.wav> [--write] [--out DIR]');
// ATTEMPT, don't ask. `existsSync` then `openSync` is a check-then-use race
// (CodeQL `js/file-system-race`, which points at the OPEN — the half that does
// the damage), and the open reports a missing file and a permission error itself.
//
// A DIRECTORY IS NOT ONE OF THOSE, which is worth the extra line: `open(2)` on a
// directory SUCCEEDS for reading on macOS, so `split-recording.mjs /etc` reached
// the `readSync` below and died with a raw EISDIR stack trace. It did that before
// this check-then-use was removed as well — `existsSync('/etc')` is true. The
// question is asked of the FD we are already holding, so it cannot be raced.
let fd;
try {
  fd = openSync(file, 'r');
} catch (e) {
  fail(e.code === 'ENOENT' ? `no such file: ${file}` : `cannot read ${file}: ${e.message}`);
}
if (!fstatSync(fd).isFile()) {
  closeSync(fd);
  fail(`not a regular file: ${file}`);
}
const head = Buffer.alloc(HEADER);
if (readSync(fd, head, 0, HEADER, 0) !== HEADER) fail('file is shorter than a WAV header');

if (head.subarray(0, 4).toString() !== 'RIFF' || head.subarray(8, 12).toString() !== 'WAVE') {
  fail('not a RIFF/WAVE file');
}
if (head.subarray(36, 40).toString() !== 'data') {
  fail('the data chunk is not at byte 36 — this tool only handles the recorder\'s own 44-byte header');
}

const declaredRiff = head.readUInt32LE(4);
const audioFormat = head.readUInt16LE(20);
const channels = head.readUInt16LE(22);
const rate = head.readUInt32LE(24);
const bits = head.readUInt16LE(34);
const declaredData = head.readUInt32LE(40);

const size = statSync(file).size;
const trueData = size - HEADER;
const frameBytes = (bits / 8) * channels;
const secs = (n) => n / frameBytes / rate;
const hhmm = (s) => `${Math.floor(s / 3600)}h ${String(Math.floor((s % 3600) / 60)).padStart(2, '0')}m`;

console.log(`${basename(file)}`);
console.log(`  format         ${audioFormat === 3 ? 'IEEE float' : `code ${audioFormat}`}, ${channels}ch, ${rate} Hz, ${bits}-bit`);
console.log(`  file           ${size.toLocaleString()} bytes`);
console.log(`  declared data  ${declaredData.toLocaleString()} bytes  (${hhmm(secs(declaredData))})`);
console.log(`  actual data    ${trueData.toLocaleString()} bytes  (${hhmm(secs(trueData))})`);

if (declaredData === trueData) {
  console.log('\n  header is correct — nothing to repair.');
  closeSync(fd);
  process.exit(0);
}

const wraps = Math.floor(trueData / 2 ** 32);
console.log(`  RIFF field     ${declaredRiff.toLocaleString()} (should be ${(size - 8).toLocaleString()})`);
console.log(
  `\n  OVERFLOWED: the declared length is the real one wrapped ${wraps} time${wraps === 1 ? '' : 's'} ` +
    `through 2^32. A reader honouring it discards ${hhmm(secs(trueData - declaredData))}.`,
);

// Segment on a whole frame, and keep each segment under what the format allows.
const perSegment = Math.floor(MAX_DATA / frameBytes) * frameBytes;
const segments = [];
for (let off = 0; off < trueData; off += perSegment) {
  segments.push({ off, len: Math.min(perSegment, trueData - off) });
}

const dir = outDir ?? dirname(file);
const stem = basename(file, extname(file));
const name = (i) => join(dir, `${stem}-part${String(i + 1).padStart(2, '0')}.wav`);

console.log(`\n  ${segments.length} segment${segments.length === 1 ? '' : 's'} at ${hhmm(secs(perSegment))} each:`);
for (const [i, s] of segments.entries()) {
  console.log(`    ${basename(name(i))}  ${hhmm(secs(s.len))}  (${s.len.toLocaleString()} bytes)`);
}

if (!write) {
  console.log('\n  DRY RUN — nothing written. Re-run with --write to produce these files.');
  console.log('  The input is never modified or deleted, with or without --write.');
  closeSync(fd);
  process.exit(0);
}

if (outDir && !existsSync(outDir)) mkdirSync(outDir, { recursive: true });
for (const [i, s] of segments.entries()) {
  const out = name(i);
  if (existsSync(out)) fail(`${out} already exists — refusing to overwrite`);
}

const header = (dataLen) => {
  const h = Buffer.alloc(HEADER);
  h.write('RIFF', 0);
  h.writeUInt32LE(36 + dataLen, 4);
  h.write('WAVE', 8);
  h.write('fmt ', 12);
  h.writeUInt32LE(16, 16);
  h.writeUInt16LE(audioFormat, 20);
  h.writeUInt16LE(channels, 22);
  h.writeUInt32LE(rate, 24);
  h.writeUInt32LE(rate * frameBytes, 28);
  h.writeUInt16LE(frameBytes, 32);
  h.writeUInt16LE(bits, 34);
  h.write('data', 36);
  h.writeUInt32LE(dataLen, 40);
  return h;
};

const BUF = 1 << 24; // 16 MiB
const buf = Buffer.alloc(BUF);
for (const [i, s] of segments.entries()) {
  const out = name(i);
  const ofd = openSync(out, 'wx');
  writeSync(ofd, header(s.len));
  let done = 0;
  while (done < s.len) {
    const want = Math.min(BUF, s.len - done);
    const got = readSync(fd, buf, 0, want, HEADER + s.off + done);
    if (got === 0) fail(`input ended early at ${HEADER + s.off + done}`);
    writeSync(ofd, buf, 0, got);
    done += got;
  }
  closeSync(ofd);
  console.log(`  wrote ${out}  ${done.toLocaleString()} bytes`);
}
closeSync(fd);
console.log(`\n  done. ${basename(file)} is untouched; delete it yourself once the segments check out.`);
