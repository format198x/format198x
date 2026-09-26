// Capture LIST output for cases.bas from the genuine 48K ROM running in
// Emu198x, one listed line per case, into cases.list. See README.md.
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

const source = fs.readFileSync(path.join(here, 'cases.bas'), 'utf8');
const numbers = source
  .split('\n')
  .filter((line) => line.trim() !== '')
  .map((line) => Number.parseInt(line, 10));

const spectrum = emu.Spectrum.createHeadless(new Uint8Array(rom));
const run = (ms) => {
  for (let t = 0; t < ms; t += 20) spectrum.tick(20);
};
// Hold 100 ms, release, wait 200 ms: the rhythm proven in the 2026-09-26 spike.
const tap = (code) => {
  spectrum.keyDown(code);
  run(100);
  spectrum.keyUp(code);
  run(200);
};
const typeNumber = (n) => {
  for (const digit of String(n)) tap(`Digit${digit}`);
};
const rows = () => JSON.parse(spectrum.query('screen.text.lines'));

run(3000);
spectrum.runBasic(source);
run(1000);
const stopped = rows()[23];
if (!stopped.startsWith('9 STOP statement')) {
  throw new Error(`expected line 1 to stop the program, lower screen shows "${stopped}"`);
}

const lineStart = (n) => `${String(n).padStart(4, ' ')} `;
const out = [];
for (const [index, n] of numbers.entries()) {
  // CLS (K mode V), so the listing starts on the top row.
  tap('KeyV');
  tap('Enter');
  run(300);
  // LIST (K mode K) from one past the previous line. LIST n would set E_PPC
  // to n and print the '>' edit cursor in place of the space after n; listing
  // from a number with no line still starts at n but marks no line.
  const from = index === 0 ? 0 : numbers[index - 1] + 1;
  tap('KeyK');
  typeNumber(from);
  tap('Enter');
  run(500);

  const screen = rows();
  if (!screen[0].startsWith(lineStart(n))) {
    throw new Error(`LIST ${from} should start with line ${n}, top row is "${screen[0]}"`);
  }
  const next = numbers[index + 1];
  let text = screen[0];
  for (let row = 1; row < 22; row += 1) {
    const r = screen[row];
    if (r.trim() === '' || (next !== undefined && r.startsWith(lineStart(next)))) break;
    if (row === 21) throw new Error(`line ${n} runs off the listing`);
    text += r;
  }
  out.push(text.trimEnd());

  if (screen.some((r) => r.includes('scroll?'))) {
    tap('KeyN');
    run(300);
  }
}

fs.writeFileSync(path.join(here, 'cases.list'), `${out.join('\n')}\n`);
console.log(`captured ${out.length} lines with @emu198x/zx-spectrum ${version}, ROM SHA1 ${sha1}`);
