import { readFileSync } from 'fs';

const extract = JSON.parse(readFileSync('/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-extract-results-16.json', 'utf8'));

// Check exports for detect.rs and error.rs
for (const file of extract.results) {
  if (file.path && (file.path.includes('detect.rs') || file.path.includes('error.rs'))) {
    console.log(`\n=== ${file.path} exports ===`);
    console.log(JSON.stringify(file.exports, null, 2));
  }
}
