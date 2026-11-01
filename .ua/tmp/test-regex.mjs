import { readFileSync } from 'fs';

const ROOT = '/home/mat/Documents/src/oxiguitar';

const structRegex = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?struct\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/;
const enumRegex = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?enum\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/;
const traitRegex = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?trait\s+(\w+)(?:\s*<[^>]*>)?\s*(?::\s*[^{]+)?\s*(?:\{)/;

const files = [
  'crates/oxiguitar-edit/src/selection.rs',
  'crates/oxiguitar-edit/src/tracks.rs',
  'crates/oxiguitar-edit/src/transpose.rs',
  'crates/oxiguitar-formats/src/detect.rs',
  'crates/oxiguitar-formats/src/error.rs',
  'crates/oxiguitar-formats/src/lib.rs',
];

for (const path of files) {
  const content = readFileSync(`${ROOT}/${path}`, 'utf8');
  const lines = content.split('\n');
  console.log(`\n=== ${path} ===`);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    const sm = line.match(structRegex);
    const em = line.match(enumRegex);
    const tm = line.match(traitRegex);
    if (sm) console.log(`  STRUCT: ${sm[1]} at line ${i+1}: ${line.substring(0, 80)}`);
    if (em) console.log(`  ENUM: ${em[1]} at line ${i+1}: ${line.substring(0, 80)}`);
    if (tm) console.log(`  TRAIT: ${tm[1]} at line ${i+1}: ${line.substring(0, 80)}`);
  }
}
