// Record the five-byte form the genuine 48K ROM computes for a spread of
// decimal numbers, via VAL, in Emu198x, into numbers.tsv. See README.md.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROM_SHA1 = '5ea7c2b824672e914525d1d5c419d71b84a426a2';
// The emulator version README.md records for this capture.
const PACKAGE_VERSION = '0.4.0';
const here = path.dirname(fileURLToPath(import.meta.url));

const pkg = process.env.EMU198X_ZX_SPECTRUM_PKG;
const romPath = process.env.SPECTRUM_48K_ROM;
if (!pkg || !romPath) {
  throw new Error('set EMU198X_ZX_SPECTRUM_PKG (a directory) and SPECTRUM_48K_ROM');
}
const rom = fs.readFileSync(romPath);
const sha1 = createHash('sha1').update(rom).digest('hex');
if (sha1 !== ROM_SHA1) {
  throw new Error(`${romPath} has SHA1 ${sha1}; the capture needs the genuine 48K ROM ${ROM_SHA1}`);
}
const emu = await import(path.join(pkg, 'emu198x_spectrum_web.js'));
emu.initSync({ module: fs.readFileSync(path.join(pkg, 'emu198x_spectrum_web_bg.wasm')) });
const version = JSON.parse(fs.readFileSync(path.join(pkg, 'package.json'), 'utf8')).version;
if (version !== PACKAGE_VERSION) {
  throw new Error(
    `${pkg} is @emu198x/zx-spectrum ${version}; README.md records ${PACKAGE_VERSION} for this capture`,
  );
}

// A fixed pseudo-random sequence, so every capture uses the same numbers.
let seed = 198;
const random = (n) => {
  seed = (seed * 1103515245 + 12345) % 2147483648;
  return Math.floor((seed / 2147483648) * n);
};
const digits = (count) => Array.from({ length: count }, () => random(10)).join('');
const spellings = new Set([
  '0', '1', '9', '10', '255', '256', '65535', '65536', '65537', '99999', '1000000',
  '.5', '.25', '.1', '0.1', '.05', '.001', '.0001', '1.5', '2.5', '3.14159', '3.14159265',
  '0.3', '0.7', '1.1', '99.99', '123.456', '0.000001', '1E0', '1E1', '1E-1', '1E38', '1.5E38',
  '1E-38', '1E-39', '2E-39', '1E-40', '1E-45', '5E-50', '.5E3', '1.E3', '12.5E2',
  '1.5E-3', '1e5', '1E+5', '2.5e-2', '4294967295', '4294967296', '12345678901234567890',
]);
const add = (s) => spellings.add(s);
for (let i = 0; i < 150; i += 1) add(String(random(65536)));
for (let i = 0; i < 150; i += 1) add(`${1 + random(9)}${digits(4 + random(14))}`);
for (let i = 0; i < 500; i += 1) add(`${random(1000)}.${digits(1 + random(9))}`);
for (let i = 0; i < 200; i += 1) add(`.${digits(1 + random(9))}`);
for (let i = 0; i < 100; i += 1) add(`${random(100)}.${digits(1 + random(3))}0`);
for (let i = 0; i < 500; i += 1) {
  const mantissas = [`${1 + random(9)}`, `${1 + random(9)}.${digits(1 + random(4))}`, `.${digits(1 + random(3))}`, `${random(1000)}`];
  const mantissa = mantissas[random(mantissas.length)];
  const sign = ['', '+', '-', '-'][random(4)];
  const e = ['E', 'e'][random(2)];
  add(`${mantissa}${e}${sign}${random(sign === '-' ? 45 : 36)}`);
}
const all = [...spellings];

// One program per batch: READ each spelling as a string and store its VAL
// in r(). VAL checks the string's syntax before evaluating it, and on that
// pass S-DECIMAL calls DEC-TO-FP, as when a line is entered.
const BATCH = 300;
const rows = [];
for (let start = 0; start < all.length; start += BATCH) {
  const batch = all.slice(start, start + BATCH);
  const data = [];
  for (let i = 0; i < batch.length; i += 12) {
    data.push(`${1000 + i} DATA ${batch.slice(i, i + 12).map((s) => `"${s}"`).join(',')}`);
  }
  const source = [
    `10 DIM r(${batch.length})`,
    `20 FOR i=1 TO ${batch.length}`,
    '30 READ s$',
    '40 LET r(i)=VAL s$',
    '50 NEXT i',
    '60 STOP',
    ...data,
  ].join('\n');
  const spectrum = emu.Spectrum.createHeadless(new Uint8Array(rom));
  const run = (ms) => {
    for (let t = 0; t < ms; t += 20) spectrum.tick(20);
  };
  run(3000);
  spectrum.runBasic(source);
  let report = '';
  for (let t = 0; t < 120000 && !/^\d+ \S/.test(report); t += 200) {
    run(200);
    report = JSON.parse(spectrum.query('screen.text.lines'))[23];
  }
  if (!report.startsWith('9 STOP statement, 60')) {
    throw new Error(`batch at ${start} ended with "${report}"`);
  }
  const peek16 = (address) => {
    const [lo, hi] = spectrum.readMemory(address, 2);
    return lo | (hi << 8);
  };
  // r() is the first variable DIM made: 0x92, length, one dimension, size.
  const vars = peek16(23627);
  const header = spectrum.readMemory(vars, 6);
  if (header[0] !== 0x92 || header[3] !== 1 || (header[4] | (header[5] << 8)) !== batch.length) {
    throw new Error(`VARS does not start with r(${batch.length})`);
  }
  const values = spectrum.readMemory(vars + 6, 5 * batch.length);
  batch.forEach((s, i) => {
    rows.push(`${s}\t${Buffer.from(values.slice(5 * i, 5 * i + 5)).toString('hex')}`);
  });
}
fs.writeFileSync(path.join(here, 'numbers.tsv'), `${rows.join('\n')}\n`);
console.log(`captured ${rows.length} numbers with @emu198x/zx-spectrum ${version}, ROM SHA1 ${sha1}`);
