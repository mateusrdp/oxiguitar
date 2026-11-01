import { readFileSync } from 'fs';

const output = JSON.parse(readFileSync('/home/mat/Documents/src/oxiguitar/.ua/intermediate/batch-16.json', 'utf8'));
const input = JSON.parse(readFileSync('/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-analyzer-input-16.json', 'utf8'));
const extract = JSON.parse(readFileSync('/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-extract-results-16.json', 'utf8'));

const inputPaths = new Set(input.batchFiles.map(f => f.path));
const nodeIds = new Set(output.nodes.map(n => n.id));

// 1. Find duplicate node IDs
const idCounts = {};
for (const n of output.nodes) {
  idCounts[n.id] = (idCounts[n.id] || 0) + 1;
}
const duplicates = Object.entries(idCounts).filter(([_, c]) => c > 1);
console.log('=== DUPLICATE NODE IDs ===');
console.log(JSON.stringify(duplicates, null, 2));

// 2. Find edges referencing non-existent nodes
const invalidEdges = output.edges.filter(e => !nodeIds.has(e.target) && !nodeIds.has(e.source));
console.log('\n=== EDGES WITH NON-EXISTENT NODES ===');
for (const e of invalidEdges) {
  console.log(`${e.type}: ${e.source} -> ${e.target}`);
}
console.log(`\nTotal invalid edges: ${invalidEdges.length}`);

// 3. Find tested_by edges referencing files not in batch
const testedByInvalid = output.edges.filter(e => e.type === 'tested_by' && !inputPaths.has(e.target));
console.log('\n=== TESTED_BY EDGES TO MISSING FILES ===');
for (const e of testedByInvalid) {
  console.log(`${e.source} -> ${e.target} (target not in batch)`);
}

// 4. Check export edges targeting wrong types
console.log('\n=== EXPORT EDGES CHECK ===');
const structNodes = new Set(output.nodes.filter(n => n.type === 'struct').map(n => n.id));
const enumNodes = new Set(output.nodes.filter(n => n.type === 'enum').map(n => n.id));
const functionNodes = new Set(output.nodes.filter(n => n.type === 'function').map(n => n.id));

for (const e of output.edges.filter(e => e.type === 'exports')) {
  const target = e.target;
  if (structNodes.has(target)) {
    console.log(`export -> struct: ${target}`);
  } else if (enumNodes.has(target)) {
    console.log(`export -> enum: ${target}`);
  } else if (functionNodes.has(target)) {
    // ok
  } else {
    console.log(`export -> UNKNOWN: ${target}`);
  }
}

// 5. Check the specific duplicate
console.log('\n=== DUPLICATE: function:crates/oxiguitar-formats/src/gpif/mod.rs:new ===');
const gpifMod = extract.results.find(r => r.path && r.path.includes('gpif/mod.rs'));
if (gpifMod) {
  console.log('Functions named "new" in gpif/mod.rs:');
  for (const f of gpifMod.functions) {
    if (f.name === 'new') console.log(`  - ${f.name} at line ${f.line}`);
  }
}

// 6. Summary stats
console.log('\n=== SUMMARY ===');
console.log(`Nodes: ${output.nodes.length}`);
console.log(`Edges: ${output.edges.length}`);
const typeCounts = {};
for (const n of output.nodes) typeCounts[n.type] = (typeCounts[n.type] || 0) + 1;
console.log('Node types:', JSON.stringify(typeCounts));
const edgeCounts = {};
for (const e of output.edges) edgeCounts[e.type] = (edgeCounts[e.type] || 0) + 1;
console.log('Edge types:', JSON.stringify(edgeCounts));
