// Test simplified regexes
const lines = [
  'pub enum Format {',
  'pub struct ReadError {',
  'pub(crate) struct Cursor<\'a> {',
  'pub(super) struct BeatNoteEffects {',
  'pub enum ReadErrorKind {',
  'pub enum Severity {',
  'pub(crate) struct GpRead {',
  'pub(super) struct Reader<\'a> {',
  'pub struct Span {',
  'pub struct TrackSettings {',
  'pub struct Transposition {',
  'pub enum Scope {',
  'pub struct Imported {',
  'pub struct Written {',
];

// Simplified regexes that handle pub, pub(crate), pub(super)
const structRegex = /^pub(?:\s*\([^)]*\)\s+)?struct\s+(\w+)/;
const enumRegex = /^pub(?:\s*\([^)]*\)\s+)?enum\s+(\w+)/;

console.log('=== Struct regex ===');
for (const line of lines) {
  const match = line.match(structRegex);
  if (match) console.log(`  STRUCT: ${match[1]} - ${line}`);
}

console.log('\n=== Enum regex ===');
for (const line of lines) {
  const match = line.match(enumRegex);
  if (match) console.log(`  ENUM: ${match[1]} - ${line}`);
}
