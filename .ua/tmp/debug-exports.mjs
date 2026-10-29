import { readFileSync } from 'fs';

const ROOT = '/home/mat/Documents/src/oxiguitar';

// Check the enum/struct lines for the problematic exports
const checks = [
  ['crates/oxiguitar-edit/src/selection.rs', 'Selection'],
  ['crates/oxiguitar-edit/src/selection.rs', 'Span'],
  ['crates/oxiguitar-edit/src/tracks.rs', 'TrackSettings'],
  ['crates/oxiguitar-edit/src/transpose.rs', 'Transposition'],
  ['crates/oxiguitar-edit/src/transpose.rs', 'Scope'],
  ['crates/oxiguitar-formats/src/detect.rs', 'Format'],
  ['crates/oxiguitar-formats/src/error.rs', 'ReadError'],
  ['crates/oxiguitar-formats/src/error.rs', 'ReadErrorKind'],
  ['crates/oxiguitar-formats/src/error.rs', 'Severity'],
  ['crates/oxiguitar-formats/src/lib.rs', 'Imported'],
  ['crates/oxiguitar-formats/src/lib.rs', 'Written'],
];

const structRegex = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?struct\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/;
const enumRegex = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?enum\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/;

for (const [path, name] of checks) {
  const content = readFileSync(`${ROOT}/${path}`, 'utf8');
  const lines = content.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    const sm = line.match(structRegex);
    const em = line.match(enumRegex);
    if (sm && sm[1] === name) {
      console.log(`${path}:${i+1}: STRUCT match: ${line}`);
    }
    if (em && em[1] === name) {
      console.log(`${path}:${i+1}: ENUM match: ${line}`);
    }
  }
}

// Also check what the lib.rs exports look like
console.log('\n=== lib.rs exports ===');
const extract = JSON.parse(readFileSync('/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-extract-results-16.json', 'utf8'));
const libExports = extract.results.find(r => r.path && r.path.includes('formats/src/lib.rs'));
if (libExports) {
  console.log(JSON.stringify(libExports.exports, null, 2));
}

// Check detect.rs module declarations
console.log('\n=== detect.rs content (first 20 lines) ===');
const detectContent = readFileSync(`${ROOT}/crates/oxiguitar-formats/src/detect.rs`, 'utf8');
console.log(detectContent.split('\n').slice(0, 20).join('\n'));

// Check error.rs module declarations
console.log('\n=== error.rs content (first 20 lines) ===');
const errorContent = readFileSync(`${ROOT}/crates/oxiguitar-formats/src/error.rs`, 'utf8');
console.log(errorContent.split('\n').slice(0, 20).join('\n'));
