// Test regex patterns
const lines = [
  'pub enum Format {',
  'pub struct ReadError {',
  'pub(crate) struct Cursor<\'a> {',
  'pub(super) struct BeatNoteEffects {',
  'pub enum ReadErrorKind {',
  'pub enum Severity {',
  'pub enum GpifRead {',
  'pub enum Imported {',
  'pub struct Span {',
  'pub struct TrackSettings {',
  'pub struct Transposition {',
  'pub enum Scope {',
];

// Original regex
const origStruct = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?struct\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/;
const origEnum = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?enum\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/;

// Fixed regex - the alternation needs proper grouping
const fixedStruct = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?struct\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{([^}]*)\}|;)/;
const fixedEnum = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?enum\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{([^}]*)\}|;)/;

console.log('=== Original regex ===');
for (const line of lines) {
  const sm = line.match(origStruct);
  const em = line.match(origEnum);
  if (sm) console.log(`STRUCT match: ${sm[1]} - ${line}`);
  if (em) console.log(`ENUM match: ${em[1]} - ${line}`);
}

console.log('\n=== Fixed regex ===');
for (const line of lines) {
  const sm = line.match(fixedStruct);
  const em = line.match(fixedEnum);
  if (sm) console.log(`STRUCT match: ${sm[1]} - ${line}`);
  if (em) console.log(`ENUM match: ${em[1]} - ${line}`);
}

// Also test with actual file content
console.log('\n=== Testing with actual file content ===');
const testContent = `pub enum Format {
    GuitarPro1to2 {
        version: String,
    },
}

pub struct ReadError {
    pub kind: ReadErrorKind,
}`;

console.log('Fixed enum matches:');
for (const line of testContent.split('\n')) {
  const em = line.match(fixedEnum);
  if (em) console.log(`  ENUM: ${em[1]}`);
}

console.log('Fixed struct matches:');
for (const line of testContent.split('\n')) {
  const sm = line.match(fixedStruct);
  if (sm) console.log(`  STRUCT: ${sm[1]}`);
}
