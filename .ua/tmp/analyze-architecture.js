#!/usr/bin/env node
// Architecture Analysis Script for OxiGuitar
// Reads file nodes and import edges, computes structural patterns,
// and assigns every node to exactly one architectural layer.

const fs = require('fs');
const path = require('path');

// --- Validate arguments ---
if (process.argv.length < 4) {
  console.error('Usage: analyze-architecture.js <input.json> <output.json>');
  process.exit(1);
}

const inputFile = process.argv[2];
const outputFile = process.argv[3];

// Determine input directory (same dir as input file)
const inputDir = path.dirname(inputFile);

let nodes, importEdges, allEdges;

try {
  const rawNodes = fs.readFileSync(inputFile, 'utf8');
  const parsed = JSON.parse(rawNodes);
  // Input may be an object with named keys or a plain array
  if (Array.isArray(parsed)) {
    nodes = parsed;
    // Try to load importEdges and allEdges from adjacent files
    const baseName = path.basename(inputFile, '.json');
    const importPath = inputDir + '/arch-import-edges.json';
    const allPath = inputDir + '/arch-all-edges.json';
    try { importEdges = JSON.parse(fs.readFileSync(importPath, 'utf8')); } catch(e) { importEdges = []; }
    try { allEdges = JSON.parse(fs.readFileSync(allPath, 'utf8')); } catch(e) { allEdges = []; }
  } else if (parsed.fileNodes) {
    nodes = parsed.fileNodes;
    importEdges = parsed.importEdges || [];
    allEdges = parsed.allEdges || [];
  } else {
    console.error('Unknown input format');
    process.exit(1);
  }
} catch (e) {
  console.error('Failed to read or parse input file:', e.message);
  process.exit(1);
}

// --- Helper: Extract file path from node ---
function getNodeFilePath(node) {
  if (node.filePath && typeof node.filePath === 'string' && node.filePath.trim()) {
    return node.filePath.trim();
  }
  // Fallback: strip prefix from node id
  const id = node.id || '';
  const prefixMatch = id.match(/^(file|config|document|pipeline|service|pipeline)\:/i);
  if (prefixMatch) {
    return id.substring(prefixMatch[0].length);
  }
  return id;
}

// --- Helper: Get top-level directory group ---
function getTopLevelGroup(filePath) {
  if (!filePath) return '__root__';
  const parts = filePath.split('/').filter(Boolean);
  return parts[0] || '__root__';
}

// --- Helper: Get second-level directory group ---
function getSecondLevelGroup(filePath) {
  if (!filePath) return '__root__';
  const parts = filePath.split('/').filter(Boolean);
  if (parts.length >= 2) return parts[0] + '/' + parts[1];
  return parts[0] || '__root__';
}

// ================================================================
// PHASE 1: Structural Analysis
// ================================================================

// A. Directory Grouping
const dirGroups = {};
nodes.forEach(node => {
  const fp = getNodeFilePath(node);
  const group = getSecondLevelGroup(fp);
  if (!dirGroups[group]) dirGroups[group] = [];
  dirGroups[group].push(node.id);
});

// B. Node Type Grouping
const typeGroups = {};
nodes.forEach(node => {
  const type = node.type || 'file';
  if (!typeGroups[type]) typeGroups[type] = [];
  typeGroups[type].push(node.id);
});

// C. Import Adjacency Matrix (fan-in/fan-out)
const fanIn = {};
const fanOut = {};
nodes.forEach(node => {
  fanIn[node.id] = 0;
  fanOut[node.id] = 0;
});

importEdges.forEach(edge => {
  if (fanOut[edge.source] !== undefined) fanOut[edge.source]++;
  if (fanIn[edge.target] !== undefined) fanIn[edge.target]++;
});

// Import groups per directory group
const dirGroupImports = {};
Object.entries(dirGroups).forEach(([group, nodeIds]) => {
  dirGroupImports[group] = {
    totalFanIn: nodeIds.reduce((sum, id) => sum + (fanIn[id] || 0), 0),
    totalFanOut: nodeIds.reduce((sum, id) => sum + (fanOut[id] || 0), 0),
    nodeCount: nodeIds.length
  };
});

// D. Cross-Category Dependency Analysis
const crossCategory = {};
importEdges.forEach(edge => {
  const srcNode = nodes.find(n => n.id === edge.source);
  const tgtNode = nodes.find(n => n.id === edge.target);
  if (!srcNode || !tgtNode) return;
  const srcType = srcNode.type || 'file';
  const tgtType = tgtNode.type || 'file';
  const key = srcType + '->' + tgtType;
  if (!crossCategory[key]) crossCategory[key] = 0;
  crossCategory[key]++;
});

// E. Inter-Group Import Frequency
const interGroupImports = {};
importEdges.forEach(edge => {
  const srcNode = nodes.find(n => n.id === edge.source);
  const tgtNode = nodes.find(n => n.id === edge.target);
  if (!srcNode || !tgtNode) return;
  const srcGroup = getSecondLevelGroup(getNodeFilePath(srcNode));
  const tgtGroup = getSecondLevelGroup(getNodeFilePath(tgtNode));
  if (srcGroup === tgtGroup) return; // skip intra-group
  const key = srcGroup + '->' + tgtGroup;
  if (!interGroupImports[key]) interGroupImports[key] = 0;
  interGroupImports[key]++;
});

// F. Intra-Group Import Density
const intraGroupDensity = {};
Object.entries(dirGroups).forEach(([group, nodeIds]) => {
  const totalEdges = nodeIds.reduce((sum, id) => sum + (fanIn[id] || 0) + (fanOut[id] || 0), 0);
  let intraEdges = 0;
  importEdges.forEach(edge => {
    const srcNode = nodes.find(n => n.id === edge.source);
    const tgtNode = nodes.find(n => n.id === edge.target);
    if (!srcNode || !tgtNode) return;
    const srcGroup = getSecondLevelGroup(getNodeFilePath(srcNode));
    const tgtGroup = getSecondLevelGroup(getNodeFilePath(tgtNode));
    if (srcGroup === group && tgtGroup === group) intraEdges++;
  });
  intraGroupDensity[group] = {
    totalEdges,
    intraEdges,
    density: totalEdges > 0 ? (intraEdges / totalEdges) : 0
  };
});

// G. Directory Pattern Matching
const patternMap = {
  'crates': 'core-libraries',
  'plugins': 'external-plugins',
  'fixtures': 'test-fixtures',
  'docs': 'documentation',
  'scripts': 'scripts',
  'spikes': 'experimental',
  'fuzz': 'fuzz-testing',
  'debian': 'packaging',
  '.github': 'ci-cd',
  'corpus': 'test-fixtures',
  'assets': 'assets'
};

const dirPatterns = {};
Object.keys(dirGroups).forEach(group => {
  const topLevel = getTopLevelGroup(group);
  dirPatterns[group] = patternMap[topLevel] || 'other';
});

// H. Deployment Topology Detection
const hasDocker = nodes.some(n => {
  const fp = getNodeFilePath(n);
  return fp.includes('Dockerfile') || fp.includes('docker-compose');
});
const hasGitHubActions = nodes.some(n => {
  const fp = getNodeFilePath(n);
  return fp.startsWith('.github/workflows/');
});
const hasDebian = nodes.some(n => {
  const fp = getNodeFilePath(n);
  return fp.startsWith('debian/');
});
const hasFuzzConfig = nodes.some(n => {
  const fp = getNodeFilePath(n);
  return fp.startsWith('fuzz/');
});

// I. Data Pipeline Detection
const hasFormatParsers = nodes.some(n => {
  const fp = getNodeFilePath(n);
  return fp.includes('oxiguitar-formats');
});
const hasModel = nodes.some(n => {
  const fp = getNodeFilePath(n);
  return fp.includes('oxiguitar-model');
});
const hasPluginApi = nodes.some(n => {
  const fp = getNodeFilePath(n);
  return fp.includes('oxiguitar-plugin-api');
});

// J. Documentation Coverage
const docFiles = new Set();
nodes.filter(n => n.type === 'document').forEach(n => {
  const fp = getNodeFilePath(n);
  if (fp.endsWith('.md') || fp.endsWith('.txt') || fp.includes('README')) {
    docFiles.add(getTopLevelGroup(fp));
  }
});

// K. Dependency Direction between groups
const depDirection = {};
importEdges.forEach(edge => {
  const srcNode = nodes.find(n => n.id === edge.source);
  const tgtNode = nodes.find(n => n.id === edge.target);
  if (!srcNode || !tgtNode) return;
  const srcGroup = getSecondLevelGroup(getNodeFilePath(srcNode));
  const tgtGroup = getSecondLevelGroup(getNodeFilePath(tgtNode));
  if (srcGroup === tgtGroup) return;
  const key = srcGroup + '->' + tgtGroup;
  if (!depDirection[key]) depDirection[key] = 0;
  depDirection[key]++;
});

// ================================================================
// PHASE 2: Semantic Layer Assignment
// ================================================================

// Layer definitions based on structural analysis
// The layers reflect OxiGuitar's Rust workspace architecture:
// 1. Core Application - main Rust crates (model, display, edit, play, etc.)
// 2. Plugin Ecosystem - external audio and format plugins
// 3. Test Infrastructure - test fixtures, fuzz targets, events
// 4. Documentation - all user-facing docs and manuals
// 5. Scripts and Spikes - build scripts, experimental code, tooling
// 6. Configuration - Cargo.toml, manifest.json, packaging config
// 7. CI/CD Pipelines - GitHub Actions workflows

const layers = [
  {
    id: 'layer:core-application',
    name: 'Core Application',
    description: 'Main Rust crates implementing OxiGuitar\'s music data model, display engine, editor, playback, and plugin API.',
    nodeIds: []
  },
  {
    id: 'layer:plugin-ecosystem',
    name: 'Plugin Ecosystem',
    description: 'External audio backend and file format plugins for SoundFont synthesis, ALSA, CoreAudio, WASAPI, PipeWire, and Guitar Pro variants.',
    nodeIds: []
  },
  {
    id: 'layer:test-infrastructure',
    name: 'Test Infrastructure',
    description: 'Test fixtures, fuzzing targets, and event data used to verify format parsers and rendering correctness.',
    nodeIds: []
  },
  {
    id: 'layer:documentation',
    name: 'Documentation',
    description: 'User manual, plugin developer guide, and project documentation in markdown and mdBook format.',
    nodeIds: []
  },
  {
    id: 'layer:scripts-and-spikes',
    name: 'Scripts and Spikes',
    description: 'Build scripts, development tooling, CI helper scripts, and experimental GUI/audio spikes.',
    nodeIds: []
  },
  {
    id: 'layer:configuration',
    name: 'Configuration',
    description: 'Cargo manifests, plugin manifests, Debian packaging config, and other project configuration files.',
    nodeIds: []
  },
  {
    id: 'layer:ci-cd-pipelines',
    name: 'CI/CD Pipelines',
    description: 'GitHub Actions workflows for CI, builds, releases, fuzzing, interop testing, and documentation deployment.',
    nodeIds: []
  }
];

// Layer classification function
function classifyNode(node) {
  const fp = getNodeFilePath(node);
  const type = node.type || 'file';
  const id = node.id || '';

  // CI/CD Pipelines: GitHub Actions workflows
  if (fp.startsWith('.github/workflows/')) return 6;
  if (type === 'pipeline') return 6;

  // Core Application: Rust crates under crates/
  if (fp.startsWith('crates/oxiguitar-')) return 0;

  // Plugin Ecosystem: external plugins
  if (fp.startsWith('plugins/')) return 1;

  // Test Infrastructure: fixtures, fuzz targets, test data
  if (fp.startsWith('fixtures/')) return 2;
  if (fp.startsWith('fuzz/')) return 2;
  if (fp.startsWith('corpus/')) return 2;

  // Scripts and Spikes: scripts/, spikes/, debian/
  if (fp.startsWith('scripts/')) return 4;
  if (fp.startsWith('spikes/')) return 4;
  if (fp.startsWith('debian/')) return 4;

  // Documentation: docs/, README files (but not in fixtures/corpus/fuzz)
  if (fp.startsWith('docs/')) return 3;
  if (type === 'document' && !fp.startsWith('fixtures/') && !fp.startsWith('corpus/') && !fp.startsWith('fuzz/')) return 3;

  // Configuration: Cargo.toml, manifest.json, .toml config, .gitattributes
  if (fp.endsWith('Cargo.toml')) return 5;
  if (fp.endsWith('manifest.json')) return 5;
  if (fp.endsWith('.toml')) return 5;
  if (fp.endsWith('.gitattributes')) return 5;
  if (fp.endsWith('.gitignore')) return 5;
  if (fp.endsWith('.slint')) return 5;

  // Default: root-level files
  return 5;
}

// Assign nodes to layers
const assigned = new Set();
nodes.forEach(node => {
  const layerIdx = classifyNode(node);
  layers[layerIdx].nodeIds.push(node.id);
  assigned.add(node.id);
});

// Verify: every node is assigned exactly once
const totalAssigned = layers.reduce((sum, l) => sum + l.nodeIds.length, 0);
if (totalAssigned !== nodes.length) {
  console.error(`ERROR: Assigned ${totalAssigned} nodes but input has ${nodes.length}`);
  process.exit(1);
}

const uniqueAssigned = new Set(layers.flatMap(l => l.nodeIds));
if (uniqueAssigned.size !== totalAssigned) {
  console.error('ERROR: Duplicate node assignments detected');
  process.exit(1);
}

// Verify: no unassigned nodes
const unassigned = nodes.filter(n => !assigned.has(n.id));
if (unassigned.length > 0) {
  console.error(`ERROR: ${unassigned.length} unassigned nodes`);
  unassigned.forEach(n => console.error('  UNASSIGNED:', n.id));
  process.exit(1);
}

// ================================================================
// Output
// ================================================================

// Build output structure
const output = {
  metadata: {
    totalNodes: nodes.length,
    totalImportEdges: importEdges.length,
    totalAllEdges: allEdges.length,
    layerCount: layers.length,
    computedAt: new Date().toISOString()
  },
  structuralAnalysis: {
    directoryGrouping: Object.fromEntries(
      Object.entries(dirGroups).map(([k, v]) => [k, { count: v.length, nodes: v }])
    ),
    nodeTypeGrouping: Object.fromEntries(
      Object.entries(typeGroups).map(([k, v]) => [k, { count: v.length }])
    ),
    importAdjacency: {
      perNode: Object.fromEntries(
        Object.entries(fanIn).map(([k, v]) => [k, { fanIn: v, fanOut: fanOut[k] || 0 }])
      ),
      perDirGroup: dirGroupImports
    },
    crossCategoryDependencies: crossCategory,
    interGroupImportFrequency: interGroupImports,
    intraGroupDensity: Object.fromEntries(
      Object.entries(intraGroupDensity).map(([k, v]) => [k, { totalEdges: v.totalEdges, intraEdges: v.intraEdges, density: v.density }])
    ),
    directoryPatterns: dirPatterns,
    deploymentTopology: {
      hasDocker,
      hasGitHubActions,
      hasDebianPackaging: hasDebian,
      hasFuzzTesting: hasFuzzConfig
    },
    dataPipelineIndicators: {
      hasFormatParsers,
      hasDataModel: hasModel,
      hasPluginApi
    },
    documentationCoverage: Object.fromEntries(
      Object.keys(dirGroups).map(g => [g, docFiles.has(getTopLevelGroup(g))])
    ),
    dependencyDirection: depDirection
  },
  layers
};

// Validate layers
layers.forEach(layer => {
  if (!layer.id.match(/^layer:[a-z][a-z0-9-]*$/)) {
    console.error('ERROR: Invalid layer id format:', layer.id);
    process.exit(1);
  }
  if (!layer.nodeIds || layer.nodeIds.length === 0) {
    console.error('ERROR: Layer', layer.id, 'has no nodes');
    process.exit(1);
  }
});

// Write output
try {
  fs.writeFileSync(outputFile, JSON.stringify(output, null, 2), 'utf8');
  console.log('Analysis complete. Written to', outputFile);
  console.log('Layers:', layers.map(l => `${l.name} (${l.nodeIds.length} nodes)`).join(', '));
  console.log('Total nodes assigned:', totalAssigned);
  process.exit(0);
} catch (e) {
  console.error('Failed to write output file:', e.message);
  process.exit(1);
}
