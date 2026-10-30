import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const UA_DIR = process.env.UA_DIR || '/home/mat/Documents/src/oxiguitar/.ua';
const manifest = JSON.parse(readFileSync(join(UA_DIR, 'intermediate/batch-15-manifest.json'), 'utf-8'));
const input = JSON.parse(readFileSync(join(UA_DIR, 'tmp/ua-file-analyzer-input-15.json'), 'utf-8'));
const batchImportData = input.batchImportData;

// Load all node files
let allNodes = [];
for (const nf of manifest.nodeFiles) {
  const data = JSON.parse(readFileSync(join(UA_DIR, `intermediate/${nf}`), 'utf-8'));
  allNodes = allNodes.concat(data.nodes);
}

// Load all edge files
let allEdges = [];
for (const ef of manifest.edgeFiles) {
  const data = JSON.parse(readFileSync(join(UA_DIR, `intermediate/${ef}`), 'utf-8'));
  allEdges = allEdges.concat(data.edges);
}

console.log(`Total nodes: ${allNodes.length}`);
console.log(`Total edges: ${allEdges.length}`);

// 1. Check all 25 file nodes present
const fileNodes = allNodes.filter(n => n.type === 'file');
console.log(`\nFile nodes: ${fileNodes.length}`);
const batchPaths = input.batchFiles.map(b => b.path);
const filePaths = fileNodes.map(n => n.filePath);
const missing = batchPaths.filter(p => !filePaths.includes(p));
if (missing.length > 0) {
  console.log('MISSING FILE NODES:', missing);
} else {
  console.log('All 25 file nodes present.');
}

// 2. Node type distribution
const nodeTypes = {};
for (const n of allNodes) nodeTypes[n.type] = (nodeTypes[n.type] || 0) + 1;
console.log('\nNode types:', nodeTypes);

// 3. Edge type distribution
const edgeTypes = {};
for (const e of allEdges) edgeTypes[e.type] = (edgeTypes[e.type] || 0) + 1;
console.log('\nEdge types:', edgeTypes);

// 4. Check all required fields on nodes
const requiredNodeFields = ['id', 'type', 'name', 'summary', 'tags', 'complexity'];
let nodeFieldErrors = 0;
for (const n of allNodes) {
  for (const f of requiredNodeFields) {
    if (n[f] === undefined) {
      console.log(`Node ${n.id} missing field: ${f}`);
      nodeFieldErrors++;
    }
  }
}
if (nodeFieldErrors === 0) console.log('\nAll nodes have required fields.');

// 5. Check all required fields on edges
const requiredEdgeFields = ['source', 'target', 'type', 'direction', 'weight'];
let edgeFieldErrors = 0;
for (const e of allEdges) {
  for (const f of requiredEdgeFields) {
    if (e[f] === undefined) {
      console.log(`Edge missing field: ${f} in ${e.source} -> ${e.target}`);
      edgeFieldErrors++;
    }
  }
}
if (edgeFieldErrors === 0) console.log('All edges have required fields.');

// 6. Check direction is always "forward"
const badDirections = allEdges.filter(e => e.direction !== 'forward');
if (badDirections.length > 0) console.log(`Bad directions: ${badDirections.length}`);
else console.log('All edges have direction "forward".');

// 7. Check for duplicate IDs
const idCounts = {};
for (const n of allNodes) idCounts[n.id] = (idCounts[n.id] || 0) + 1;
const dupes = Object.entries(idCounts).filter(([, count]) => count > 1);
if (dupes.length > 0) {
  console.log('\nDUPLICATE IDs:', dupes);
} else {
  console.log('No duplicate IDs.');
}

// 8. Check for self-referencing edges
const selfRefs = allEdges.filter(e => e.source === e.target);
if (selfRefs.length > 0) {
  console.log('SELF-REFERENCING EDGES:', selfRefs);
} else {
  console.log('No self-referencing edges.');
}

// 9. Validate import edge counts
let importErrors = 0;
for (const [filePath, imports] of Object.entries(batchImportData)) {
  const importEdges = allEdges.filter(e => e.type === 'imports' && e.source === `file:${filePath}`);
  if (importEdges.length !== imports.length) {
    console.log(`IMPORT MISMATCH: ${filePath}: expected ${imports.length} imports, got ${importEdges.length}`);
    importErrors++;
  }
}
if (importErrors === 0) console.log('All import edge counts match batchImportData.');

// 10. Check tested_by edges
const testedByEdges = allEdges.filter(e => e.type === 'tested_by');
console.log(`\ntested_by edges: ${testedByEdges.length}`);
for (const e of testedByEdges) {
  console.log(`  ${e.source} -> ${e.target}`);
}

console.log('\n=== Validation complete ===');
