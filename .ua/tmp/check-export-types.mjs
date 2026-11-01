import { readFileSync } from 'fs';

const extract = JSON.parse(readFileSync('/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-extract-results-16.json', 'utf8'));

// For each export target that's UNKNOWN, check what type it actually is in the source
const unknownExports = [
  ['crates/oxiguitar-edit/src/selection.rs', 'Selection'],
  ['crates/oxiguitar-edit/src/selection.rs', 'Span'],
  ['crates/oxiguitar-edit/src/tracks.rs', 'TrackSettings'],
  ['crates/oxiguitar-edit/src/transpose.rs', 'Transposition'],
  ['crates/oxiguitar-edit/src/transpose.rs', 'Scope'],
  ['crates/oxiguitar-formats/src/binary.rs', 'Cursor'],
  ['crates/oxiguitar-formats/src/container.rs', 'GpFiles'],
  ['crates/oxiguitar-formats/src/detect.rs', 'Format'],
  ['crates/oxiguitar-formats/src/error.rs', 'ReadError'],
  ['crates/oxiguitar-formats/src/error.rs', 'ReadErrorKind'],
  ['crates/oxiguitar-formats/src/error.rs', 'Severity'],
  ['crates/oxiguitar-formats/src/error.rs', 'LogEntry'],
  ['crates/oxiguitar-formats/src/error.rs', 'ImportLog'],
  ['crates/oxiguitar-formats/src/gp/effects.rs', 'BeatNoteEffects'],
  ['crates/oxiguitar-formats/src/gp/mod.rs', 'GpRead'],
  ['crates/oxiguitar-formats/src/gp/write.rs', 'Gp5Written'],
  ['crates/oxiguitar-formats/src/gpif/mod.rs', 'GpifRead'],
  ['crates/oxiguitar-formats/src/gpif/mod.rs', 'Reader'],
  ['crates/oxiguitar-formats/src/gpif/write/chord.rs', 'NoteName'],
  ['crates/oxiguitar-formats/src/gpif/write/chord.rs', 'Degree'],
  ['crates/oxiguitar-formats/src/gpif/write/chord.rs', 'Spelling'],
  ['crates/oxiguitar-formats/src/gpif/write/mod.rs', 'GpWritten'],
  ['crates/oxiguitar-formats/src/gpif/write/tables.rs', 'Sound'],
  ['crates/oxiguitar-formats/src/gpif/write/tables.rs', 'DrumElement'],
  ['crates/oxiguitar-formats/src/gpif/write/tables.rs', 'Articulation'],
  ['crates/oxiguitar-formats/src/lib.rs', 'Imported'],
  ['crates/oxiguitar-formats/src/lib.rs', 'Written'],
];

const ROOT = '/home/mat/Documents/src/oxiguitar';
const structRegex = /^pub\s+(?:\s*(?:#\[[^\]]*\])\s*)?struct\s+(\w+)/;
const enumRegex = /^pub\s+(?:\s*(?:#\[[^\]]*\])\s*)?enum\s+(\w+)/;
const traitRegex = /^pub\s+(?:\s*(?:#\[[^\]]*\])\s*)?trait\s+(\w+)/;

for (const [path, name] of unknownExports) {
  const content = readFileSync(`${ROOT}/${path}`, 'utf8');
  const lines = content.split('\n');
  let found = null;
  for (const line of lines) {
    const trimmed = line.trim();
    const sm = trimmed.match(structRegex);
    const em = trimmed.match(enumRegex);
    const tm = trimmed.match(traitRegex);
    if (sm && sm[1] === name) found = 'struct';
    else if (em && em[1] === name) found = 'enum';
    else if (tm && tm[1] === name) found = 'trait';
  }
  console.log(`${path}:${name} -> ${found || 'NOT_FOUND'}`);
}

// Also check the file-level exports
console.log('\n=== File-level exports ===');
const fileExports = [
  ['crates/oxiguitar-formats/src/detect.rs', 'detect'],
  ['crates/oxiguitar-formats/src/error.rs', 'error'],
];
for (const [path, name] of fileExports) {
  console.log(`${path}:${name} -> file node`);
}
