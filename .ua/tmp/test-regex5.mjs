// Debug enum regex
const lines = [
  'pub enum Format {',
  'pub enum ReadErrorKind {',
  'pub enum Severity {',
];

const regex = /^pub(?:\s*\([^)]*\)\s+)?enum\s+(\w+)/;

console.log('Testing enum regex:');
for (const line of lines) {
  const match = line.match(regex);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}

// Try without the optional group
const regex2 = /^pub\s+enum\s+(\w+)/;
console.log('\nTesting without optional group:');
for (const line of lines) {
  const match = line.match(regex2);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}

// What about pub(crate)?
const lines2 = [
  'pub(crate) enum Foo {',
  'pub(super) enum Bar {',
];
console.log('\nTesting pub(crate) enum:');
for (const line of lines2) {
  const match = line.match(regex2);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}

const regex3 = /^pub\s*\([^)]*\)\s+enum\s+(\w+)/;
console.log('\nTesting pub(...) enum regex:');
for (const line of lines2) {
  const match = line.match(regex3);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}

// Combined
const regex4 = /^pub(?:\s*\([^)]*\)\s+)?\s*enum\s+(\w+)/;
console.log('\nTesting combined regex:');
for (const line of [...lines, ...lines2]) {
  const match = line.match(regex4);
  console.log(`  "${line}" -> ${match ? match[1] : 'null'}`);
}
