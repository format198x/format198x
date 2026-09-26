// Type BASIC lines into the genuine 48K ROM's own line editor, key by key, in
// Emu198x, and record what the ROM stores and prints. See README.md.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROM_SHA1 = '5ea7c2b824672e914525d1d5c419d71b84a426a2';
// The emulator version README.md records for this capture.
const PACKAGE_VERSION = '0.4.0';
const here = path.dirname(fileURLToPath(import.meta.url));

// Each line as typed. `{KW}` is a keyword entered with its key, as a
// Spectrum owner types it; everything else is typed character by character.
// No space is typed after the line number: the editor would store one, where
// this crate stores none for the space LIST prints there. In cases.bas, the
// line number gets that space and `{KW}` becomes `KW ` (the lexer absorbs
// the one space after a keyword).
const PROGRAM = [
  '5{CLS}',
  '10{PRINT}1.5 E3;" ";1. 5;" ";12.5 E 2',
  '20{LET}a 1=2:{LET}b c=3:{PRINT}a1;" ";a 1;" ";bc',
  '30{PRINT}{BIN}1 0 1 ;" ";7 ;" ";1.5E+ 3',
  '40{PRINT}1 :{PRINT}2',
];
// Lines the ROM's syntax check refuses: a space inside a number's whole part
// or its exponent digits ends the number.
const REFUSED = ['10{PRINT}1 000', '10{PRINT}1 .5', '10{PRINT}1E3 3'];

// K-mode keys, and E (extended) mode for BIN.
const KEYWORDS = { CLS: ['KeyV'], PRINT: ['KeyP'], LET: ['KeyL'], RUN: ['KeyR'], BIN: ['ext', 'KeyB'] };
const SYMBOLS = { '=': 'L', ':': 'Z', '.': 'M', '+': 'K', '-': 'J', ';': 'O', '"': 'P', ',': 'N' };
const listingText = (typed) =>
  typed.replace(/^(\d+)/, '$1 ').replace(/\{([A-Z]+)\}/g, '$1 ').trimEnd();

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

const boot = () => {
  const spectrum = emu.Spectrum.createHeadless(new Uint8Array(rom));
  const run = (ms) => {
    for (let t = 0; t < ms; t += 20) spectrum.tick(20);
  };
  // Hold 100 ms, release, wait 200 ms: the rhythm the LIST capture uses.
  const chord = (codes) => {
    for (const code of codes) spectrum.keyDown(code);
    run(100);
    for (const code of [...codes].reverse()) spectrum.keyUp(code);
    run(200);
  };
  const press = (codes) => {
    if (codes[0] === 'ext') {
      chord(['ShiftLeft', 'ControlLeft']);
      chord(codes.slice(1));
    } else {
      chord(codes);
    }
  };
  const typeChar = (ch) => {
    if (/[0-9]/.test(ch)) return press([`Digit${ch}`]);
    if (/[a-z]/.test(ch)) return press([`Key${ch.toUpperCase()}`]);
    if (/[A-Z]/.test(ch)) return press(['ShiftLeft', `Key${ch}`]);
    if (ch === ' ') return press(['Space']);
    if (SYMBOLS[ch]) return press(['ControlLeft', `Key${SYMBOLS[ch]}`]);
    throw new Error(`no key for ${JSON.stringify(ch)}`);
  };
  const typeLine = (typed) => {
    for (const part of typed.split(/(\{[A-Z]+\})/)) {
      const keyword = part.match(/^\{([A-Z]+)\}$/);
      if (keyword) press(KEYWORDS[keyword[1]]);
      else for (const ch of part) typeChar(ch);
    }
    press(['Enter']);
    run(300);
  };
  const peek16 = (address) => {
    const [lo, hi] = spectrum.readMemory(address, 2);
    return lo | (hi << 8);
  };
  // PROG (23635) to VARS (23627): the stored program.
  const program = () => {
    const prog = peek16(23635);
    return spectrum.readMemory(prog, peek16(23627) - prog);
  };
  const rows = () => JSON.parse(spectrum.query('screen.text.lines'));
  // Run until the report appears on the bottom row.
  const untilReport = () => {
    for (let t = 0; t < 60000; t += 100) {
      run(100);
      if (/^\d+ \S/.test(rows()[23])) return rows();
    }
    throw new Error(`no report; the screen shows\n${rows().join('\n')}`);
  };
  run(3000);
  return { spectrum, run, press, typeLine, program, rows, untilReport };
};
const upperScreen = (screen) => {
  const lines = screen.map((r) => r.trimEnd());
  while (lines.length && lines[lines.length - 1] === '') lines.pop();
  return lines.join('\n');
};

// The stored lines, as the ROM's editor made them.
const machine = boot();
const hex = [];
for (const typed of PROGRAM) {
  const before = machine.program().length;
  machine.typeLine(typed);
  if (machine.program().length === before) {
    throw new Error(`the ROM refused ${typed}: ${machine.rows()[23]}`);
  }
}
const stored = machine.program();
for (let at = 0; at < stored.length; ) {
  const length = stored[at + 2] | (stored[at + 3] << 8);
  const line = stored.slice(at, at + 4 + length);
  hex.push(Buffer.from(line).toString('hex'));
  at += 4 + length;
}
if (hex.length !== PROGRAM.length) throw new Error(`stored ${hex.length} lines, typed ${PROGRAM.length}`);

// RUN the typed program and read the screen.
machine.press(KEYWORDS.RUN);
machine.press(['Enter']);
const ranTyped = upperScreen(machine.untilReport());

// Lines the ROM will not store: typing them leaves the program unchanged.
for (const typed of REFUSED) {
  const fresh = boot();
  fresh.typeLine(typed);
  if (fresh.program().length !== 0) throw new Error(`the ROM stored ${typed}`);
}

const bas = `${PROGRAM.map(listingText).join('\n')}\n`;
fs.writeFileSync(path.join(here, 'cases.bas'), bas);
fs.writeFileSync(path.join(here, 'cases.hex'), `${hex.join('\n')}\n`);
fs.writeFileSync(path.join(here, 'refused.bas'), `${REFUSED.map(listingText).join('\n')}\n`);
fs.writeFileSync(path.join(here, 'run.txt'), `${ranTyped}\n`);

// Now this crate's bytes: tokenise cases.bas, save them as a TAP that
// auto-runs from its first line, LOAD "" it on a fresh machine and compare
// what it prints with what the typed program printed.
const env = { ...process.env };
delete env.RUSTUP_TOOLCHAIN;
const cargo = spawnSync(
  'cargo',
  ['run', '-q', '-p', 'format198x-sinclair-zx-spectrum-bas', '--example', 'tokenise_listing', '--', path.join(here, 'cases.bas')],
  { env, maxBuffer: 1 << 20 },
);
if (cargo.status !== 0) throw new Error(`cargo failed: ${cargo.stderr}`);
const ours = new Uint8Array(cargo.stdout);
const block = (flag, data) => {
  const body = [flag, ...data];
  body.push(body.reduce((x, b) => x ^ b, 0));
  return [body.length & 0xff, body.length >> 8, ...body];
};
const header = [0, ...Buffer.from('cases     '), ours.length & 0xff, ours.length >> 8, 5, 0, ours.length & 0xff, ours.length >> 8];
const tap = new Uint8Array([...block(0x00, header), ...block(0xff, ours)]);
const loaded = boot();
loaded.spectrum.load(loaded.spectrum.mediaSlots()[0], 'tape', tap);
loaded.spectrum.autoload(400);
const ranOurs = upperScreen(loaded.untilReport());
if (ranOurs !== ranTyped) {
  throw new Error(`this crate's bytes print\n${ranOurs}\nbut the typed program prints\n${ranTyped}`);
}
console.log(
  `captured ${hex.length} lines and ${REFUSED.length} refused lines with @emu198x/zx-spectrum ${version}, ROM SHA1 ${sha1}; this crate's TAP prints the same`,
);
