#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'fs';

const input = JSON.parse(readFileSync('/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-extract-results-34.json', 'utf8'));
const results = input.results;

const nodes = [];
const edges = [];
const nodeIds = new Set();

// Track which files are config vs code
const configPatterns = ['Cargo.toml', 'manifest.json', 'package.json'];
const docPatterns = ['README.md'];

function isConfig(path) {
  return configPatterns.some(p => path.endsWith(p));
}

function isDoc(path) {
  return docPatterns.some(p => path.endsWith(p));
}

// Process each file
for (const file of results) {
  const path = file.path;
  const category = file.fileCategory;
  let nodeId;

  if (isConfig(path)) {
    nodeId = `config:${path}`;
  } else if (isDoc(path)) {
    nodeId = `document:${path}`;
  } else {
    nodeId = `file:${path}`;
  }

  if (!nodeIds.has(nodeId)) {
    nodeIds.add(nodeId);
    nodes.push({
      id: nodeId,
      type: isConfig(path) ? 'config' : isDoc(path) ? 'document' : 'file',
      label: path.split('/').pop(),
      path: path,
      language: file.language,
      sizeLines: file.totalLines || 0,
    });
  }

  // Extract structs/classes
  if (file.classes && file.classes.length > 0) {
    for (const cls of file.classes) {
      const structId = `struct:${path}:${cls.name}`;
      if (!nodeIds.has(structId)) {
        nodeIds.add(structId);
        nodes.push({
          id: structId,
          type: 'struct',
          label: cls.name,
          path: path,
          startLine: cls.startLine,
          endLine: cls.endLine,
          fieldCount: (cls.properties || []).length,
        });
        edges.push({
          source: nodeId,
          target: structId,
          type: 'contains',
          weight: 1.0,
        });
      }
    }
  }

  // Extract functions
  if (file.functions && file.functions.length > 0) {
    for (const fn of file.functions) {
      // Skip test functions and trivial ones
      const name = fn.name;
      if (name.startsWith('test_') || name.startsWith('the_') || name.startsWith('what_') ||
          name.startsWith('an_') || name.startsWith('control_') || name.startsWith('volume_') ||
          name.startsWith('curves_') || name.startsWith('regions_') || name.startsWith('survey')) {
        continue;
      }
      const fnId = `function:${path}:${name}`;
      if (!nodeIds.has(fnId)) {
        nodeIds.add(fnId);
        nodes.push({
          id: fnId,
          type: 'function',
          label: name,
          path: path,
          startLine: fn.startLine,
          endLine: fn.endLine,
          paramCount: (fn.params || []).length,
        });
        edges.push({
          source: nodeId,
          target: fnId,
          type: 'contains',
          weight: 1.0,
        });
      }
    }
  }

  // Extract enums
  if (file.enums && file.enums.length > 0) {
    for (const en of file.enums) {
      const enumId = `struct:${path}:${en.name}`;
      if (!nodeIds.has(enumId)) {
        nodeIds.add(enumId);
        nodes.push({
          id: enumId,
          type: 'struct',
          label: en.name,
          path: path,
          startLine: en.startLine,
          endLine: en.endLine,
        });
        edges.push({
          source: nodeId,
          target: enumId,
          type: 'contains',
          weight: 1.0,
        });
      }
    }
  }

  // Extract traits
  if (file.traits && file.traits.length > 0) {
    for (const tr of file.traits) {
      const traitId = `struct:${path}:${tr.name}`;
      if (!nodeIds.has(traitId)) {
        nodeIds.add(traitId);
        nodes.push({
          id: traitId,
          type: 'struct',
          label: tr.name,
          path: path,
          startLine: tr.startLine,
          endLine: tr.endLine,
        });
        edges.push({
          source: nodeId,
          target: traitId,
          type: 'contains',
          weight: 1.0,
        });
      }
    }
  }

  // Collect configures edge candidates (deferred to second pass)
  if (path.endsWith('Cargo.toml')) {
    const libPath = path.replace('Cargo.toml', 'src/lib.rs');
    const libId = `file:${libPath}`;
    edges.push({
      source: `config:${path}`,
      target: libId,
      type: 'configures',
      weight: 0.6,
      _deferred: true,
    });
  }
}

// Second pass: create deferred configures edges now that all nodes are registered
for (let i = edges.length - 1; i >= 0; i--) {
  const edge = edges[i];
  if (edge._deferred) {
    if (nodeIds.has(edge.target)) {
      delete edge._deferred;
    } else {
      edges.splice(i, 1);
    }
  }
}

// Also handle files that were skipped (licenses.hbs) - still add as a node
// Check the batch files list for completeness
const batchFiles = input.batchFiles || [];
for (const bf of batchFiles) {
  const p = bf.path;
  let nodeId;
  if (isConfig(p)) {
    nodeId = `config:${p}`;
  } else if (isDoc(p)) {
    nodeId = `document:${p}`;
  } else {
    nodeId = `file:${p}`;
  }
  if (!nodeIds.has(nodeId)) {
    nodeIds.add(nodeId);
    nodes.push({
      id: nodeId,
      type: isConfig(p) ? 'config' : isDoc(p) ? 'document' : 'file',
      label: p.split('/').pop(),
      path: p,
      language: bf.language,
      sizeLines: bf.sizeLines || 0,
    });
  }
}

// Add interop package.json as config
const pkgJson = 'scripts/interop/package.json';
if (!nodeIds.has(`config:${pkgJson}`)) {
  nodeIds.add(`config:${pkgJson}`);
  nodes.push({
    id: `config:${pkgJson}`,
    type: 'config',
    label: 'package.json',
    path: pkgJson,
    language: 'json',
    sizeLines: 9,
  });
}

// Write output
const output = {
  nodes: nodes,
  edges: edges,
  metadata: {
    batchNumber: 34,
    totalNodes: nodes.length,
    totalEdges: edges.length,
    filesAnalyzed: results.length,
    filesSkipped: input.filesSkipped || [],
  },
};

writeFileSync('/home/mat/Documents/src/oxiguitar/.ua/intermediate/batch-34.json', JSON.stringify(output, null, 2));
console.log(`Nodes: ${nodes.length}, Edges: ${edges.length}`);
