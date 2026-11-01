// Debug regex matching
const lines = [
  'pub enum Format {',
  'pub struct ReadError {',
  'pub(crate) struct Cursor<\'a> {',
  'pub(super) struct BeatNoteEffects {',
  'pub enum ReadErrorKind {',
];

const regex = /^(?:pub\s*\([^)]*\)\s+)?(?:\s*(?:#\[[^\]]*\])\s*)?enum\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/;

console.log('Testing enum regex:');
for (const line of lines) {
  const result = regex.test(line);
  const match = line.match(regex);
  console.log(`  "${line}" -> test: ${result}, match: ${match ? match[1] : 'null'}`);
}

// Simpler test
const simpleEnum = /^pub\s+enum\s+(\w+)/;
console.log('\nSimple enum regex:');
for (const line of lines) {
  const match = line.match(simpleEnum);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}

// What about pub(crate)?
const pubCrate = /^pub\s*\([^)]*\)\s+enum\s+(\w+)/;
console.log('\npub(crate) enum regex:');
for (const line of lines) {
  const match = line.match(pubCrate);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}

// Combined
const combined = /^(?:pub\s*\([^)]*\)\s+)?pub\s+enum\s+(\w+)/;
console.log('\nCombined pub regex:');
for (const line of lines) {
  const match = line.match(combined);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}

// Even simpler
const simplest = /^pub.*enum\s+(\w+)/;
console.log('\nSimplest pub.*enum regex:');
for (const line of lines) {
  const match = line.match(simplest);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}
