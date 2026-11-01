import { readFileSync } from 'fs';

const ROOT = '/home/mat/Documents/src/oxiguitar';
const checks = [
  ['crates/oxiguitar-formats/src/binary.rs', 'Cursor'],
  ['crates/oxiguitar-formats/src/container.rs', 'GpFiles'],
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
];

for (const [path, name] of checks) {
  const content = readFileSync(`${ROOT}/${path}`, 'utf8');
  const lines = content.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (line.includes(name) && (line.includes('struct') || line.includes('enum') || line.includes('trait'))) {
      console.log(`${path}:${i+1}: ${line}`);
    }
  }
  console.log('---');
}
