import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const UA_DIR = process.env.UA_DIR || '/home/mat/Documents/src/oxiguitar/.ua';

const data = JSON.parse(readFileSync(join(UA_DIR, 'intermediate/batch-15.json'), 'utf-8'));
const { nodes, edges } = data;

const MAX_NODES = 60;
const MAX_EDGES = 120;

// Split nodes
const nodeChunks = [];
for (let i = 0; i < nodes.length; i += MAX_NODES) {
  nodeChunks.push(nodes.slice(i, i + MAX_NODES));
}

// Split edges
const edgeChunks = [];
for (let i = 0; i < edges.length; i += MAX_EDGES) {
  edgeChunks.push(edges.slice(i, i + MAX_EDGES));
}

// Write node files
nodeChunks.forEach((chunk, idx) => {
  const outFile = join(UA_DIR, `intermediate/batch-15-nodes-${idx + 1}.json`);
  writeFileSync(outFile, JSON.stringify({ nodes: chunk }, null, 2), 'utf-8');
  console.log(`Written: ${outFile} (${chunk.length} nodes)`);
});

// Write edge files
edgeChunks.forEach((chunk, idx) => {
  const outFile = join(UA_DIR, `intermediate/batch-15-edges-${idx + 1}.json`);
  writeFileSync(outFile, JSON.stringify({ edges: chunk }, null, 2), 'utf-8');
  console.log(`Written: ${outFile} (${chunk.length} edges)`);
});

// Write manifest
const manifest = {
  batch: 15,
  totalNodes: nodes.length,
  totalEdges: edges.length,
  nodeFiles: nodeChunks.map((_, idx) => `batch-15-nodes-${idx + 1}.json`),
  edgeFiles: edgeChunks.map((_, idx) => `batch-15-edges-${idx + 1}.json`),
};
const manifestPath = join(UA_DIR, 'intermediate/batch-15-manifest.json');
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2), 'utf-8');
console.log(`\nManifest: ${manifestPath}`);
console.log(`Total: ${nodes.length} nodes, ${edges.length} edges`);
