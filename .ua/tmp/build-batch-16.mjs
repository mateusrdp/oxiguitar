#!/usr/bin/env node
/**
 * Build batch-16.json knowledge graph from extraction results + source parsing.
 * Extracts: file nodes, function nodes (from extraction), struct/enum/traits/modules (from source),
 * config nodes, and generates edges (contains, exports, configures, tested_by, calls).
 */

import { readFileSync, writeFileSync } from 'fs';
import { join, dirname, basename } from 'path';

const ROOT = '/home/mat/Documents/src/oxiguitar';
const INPUT = join(ROOT, '.ua/tmp/ua-file-analyzer-input-16.json');
const EXTRACT = join(ROOT, '.ua/tmp/ua-file-extract-results-16.json');
const OUTPUT = join(ROOT, '.ua/intermediate/batch-16.json');

const input = JSON.parse(readFileSync(INPUT, 'utf8'));
const extract = JSON.parse(readFileSync(EXTRACT, 'utf8'));
const files = input.batchFiles;
const results = extract.results;

const nodes = [];
const edges = [];
const inputPaths = new Set(files.map(f => f.path));

// Helper: build node ID
function nodeId(type, filePath, name) {
  const n = name ? `:${name}` : '';
  return `${type}:${filePath}${n}`;
}

// Helper: determine complexity based on line count and function count
function complexity(lines, funcCount) {
  const total = lines + funcCount * 5;
  if (total < 100) return 'simple';
  if (total < 500) return 'moderate';
  return 'complex';
}

// Helper: generate summary for a function
function funcSummary(name, callGraph, filePath) {
  const base = `Function \`${name}\` in ${basename(filePath)}.`;
  if (callGraph && callGraph.length > 0) {
    const targets = callGraph.slice(0, 5).map(c => c.target);
    return `${base} Calls: ${targets.join(', ')}.`;
  }
  return base;
}

// Helper: generate summary for a struct/enum/traits
function typeSummary(name, kind, filePath, fields) {
  const fieldDesc = fields && fields.length > 0 ? ` with fields: ${fields.join(', ')}.` : '.';
  return `Data structure \`${name}\` in ${basename(filePath)}${fieldDesc}`;
}

// --- Determine file category and tags ---
function fileInfo(path) {
  const base = basename(path);
  const isTest = path.includes('/tests/') || path.includes('/test/');
  const isConfig = path.endsWith('.toml') || path.endsWith('.json') || path.endsWith('.yaml');
  const crate = path.split('/')[1]; // crates/oxiguitar-edit or crates/oxiguitar-formats
  const sub = path.split('/')[2]; // oxiguitar-edit or oxiguitar-formats
  const modPath = path.replace(`${sub}/`, '');

  // Tags based on crate and module
  let tags = [];
  if (crate === 'oxiguitar-cli') tags.push('cli-tool', 'command-line');
  else if (crate === 'oxiguitar-display') tags.push('display', 'rendering');
  else if (crate === 'oxiguitar-edit') tags.push('editor', 'editing');
  else if (crate === 'oxiguitar-formats') {
    if (modPath.startsWith('gp/')) tags.push('gp-format', 'guitar-pro', 'binary-parser');
    else if (modPath.startsWith('gpif/')) tags.push('gpif-format', 'guitar-pro-interactive', 'xml-parser');
    else if (modPath === 'binary.rs') tags.push('binary-io', 'low-level');
    else if (modPath === 'detect.rs') tags.push('format-detection', 'file-type');
    else if (modPath === 'error.rs') tags.push('error-handling');
    else if (modPath === 'container.rs') tags.push('file-container', 'archive');
    else if (modPath === 'plugin.rs') tags.push('plugin-system');
    else if (modPath === 'lib.rs') tags.push('crate-root', 'module-organization');
    else if (modPath.startsWith('examples/')) tags.push('example', 'demonstration');
    else tags.push('rust-library');
  }
  if (isTest) tags.push('test', 'integration-test');
  if (isConfig) tags.push('build-config', 'rust-crate', 'dependency-management');
  if (base === 'lib.rs') tags.push('crate-interface', 'module-organization');
  if (base === 'main.rs') tags.push('entry-point', 'application-main');

  return { isTest, isConfig };
}

// --- Parse Rust source for structs, enums, traits, modules, imports ---
function parseRustSource(filePath, content) {
  const lines = content.split('\n');
  const structs = [];
  const enums = [];
  const traits = [];
  const modules = [];
  const imports = [];
  const impls = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();

    // Imports
    const impMatch = line.match(/^(?:pub\s+)?use\s+([^;]+);/);
    if (impMatch) {
      imports.push(impMatch[1].trim());
    }

    // pub use (re-exports)
    const pubUseMatch = line.match(/^(?:pub\s+)?use\s+([^;]+);/);

    // Structs (match pub, pub(crate), pub(super), etc., with optional attributes)
    const structMatch = line.match(/^(?:\s*#\[[^\]]*\]\s*)*pub(?:\s*\([^)]*\)\s+)?\s*struct\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/);
    if (structMatch) {
      const name = structMatch[1];
      const fieldsStr = structMatch[2];
      const fields = fieldsStr ? fieldsStr.split(',').map(f => f.trim().split(':')[0].trim().split(' ').pop()).filter(Boolean) : [];
      structs.push({ name, fields, line: i + 1 });
    }

    // Enums (match pub, pub(crate), pub(super), etc., with optional attributes)
    const enumMatch = line.match(/^(?:\s*#\[[^\]]*\]\s*)*pub(?:\s*\([^)]*\)\s+)?\s*enum\s+(\w+)(?:\s*<[^>]*>)?\s*(?:\{(.*)|;)/);
    if (enumMatch) {
      const name = enumMatch[1];
      // Count variants (lines between { and })
      let variants = [];
      let braceCount = 0;
      let started = false;
      for (let j = i; j < lines.length && j < i + 30; j++) {
        const l = lines[j].trim();
        if (l.includes('{')) { braceCount++; started = true; }
        if (l.includes('}')) braceCount--;
        if (started && l && !l.startsWith('//') && l !== '{' && l !== '}' && braceCount > 0) {
          const vMatch = l.match(/(\w+)/);
          if (vMatch) variants.push(vMatch[1]);
        }
        if (started && braceCount <= 0) break;
      }
      enums.push({ name, variants, line: i + 1 });
    }

    // Traits (match pub, pub(crate), pub(super), etc., with optional attributes)
    const traitMatch = line.match(/^(?:\s*#\[[^\]]*\]\s*)*pub(?:\s*\([^)]*\)\s+)?\s*trait\s+(\w+)(?:\s*<[^>]*>)?\s*(?::\s*[^{]+)?\s*(?:\{)/);
    if (traitMatch) {
      traits.push({ name: traitMatch[1], line: i + 1 });
    }

    // Modules (mod declarations)
    const modMatch = line.match(/^pub\s+mod\s+(\w+)(?:\s*{|;)/);
    if (modMatch) {
      modules.push({ name: modMatch[1], line: i + 1 });
    }

    // impl blocks
    const implMatch = line.match(/^impl(?:\s*<[^>]*>)?\s+for\s+(\w+)/);
    if (implMatch) {
      impls.push({ target: implMatch[1], line: i + 1 });
    }
    const implSelfMatch = line.match(/^impl(?:\s*<[^>]*>)?\s*(?:\bfor\s+\w+\s*)?(?:for\s+)?(\w+)(?:\s*<[^>]*>)?\s*\{/);
    if (implMatch) { /* already caught */ }
  }

  return { structs, enums, traits, modules, imports, impls };
}

// --- Parse TOML config ---
function parseToml(filePath, content) {
  const deps = [];
  const features = [];
  const lines = content.split('\n');
  let section = '';

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('[')) {
      section = trimmed;
      if (trimmed === '[features]') {
        const featMatch = trimmed.match(/\[(\w+)\]/);
        if (featMatch) features.push(featMatch[1]);
      }
    }
    if (section === '[dependencies]' || section === '[dev-dependencies]' || section === '[build-dependencies]') {
      const depMatch = trimmed.match(/^(\w+)\s*=/);
      if (depMatch) deps.push(depMatch[1]);
    }
    if (section === '[features]') {
      const featMatch = trimmed.match(/^(\w+)\s*=/);
      if (featMatch) features.push(featMatch[1]);
    }
  }

  return { deps, features };
}

// --- Process each file ---
for (let fi = 0; fi < files.length; fi++) {
  const file = files[fi];
  const path = file.path;
  const extractFile = results[fi];

  // Read source content
  let content = '';
  try {
    content = readFileSync(join(ROOT, path), 'utf8');
  } catch (e) {
    console.error(`Warning: Cannot read ${path}: ${e.message}`);
    continue;
  }

  const { isTest, isConfig } = fileInfo(path);
  const totalLines = file.sizeLines || content.split('\n').length;

  // --- File node ---
  const fileSummary = isConfig
    ? `Configuration file for the ${path.split('/')[1]} crate, defining dependencies, features, and build settings.`
    : `Source file ${basename(path)} in the ${path.split('/')[1]} crate, providing ${path.includes('gp/') ? 'GP file format support' : path.includes('gpif/') ? 'GPIF file format support' : path.includes('binary') ? 'binary I/O utilities' : path.includes('detect') ? 'format detection' : path.includes('error') ? 'error handling' : path.includes('container') ? 'file container management' : path.includes('plugin') ? 'plugin system support' : path.includes('examples') ? 'example code' : 'library functionality'}.`;

  nodes.push({
    id: nodeId('file', path),
    type: 'file',
    name: basename(path),
    filePath: path,
    summary: fileSummary,
    tags: [...new Set([...(file.tags || [])])],
    complexity: complexity(totalLines, extractFile?.metrics?.functionCount || 0)
  });

  // --- Config node (for Cargo.toml) ---
  if (isConfig) {
    const tomlData = parseToml(path, content);
    nodes.push({
      id: nodeId('config', path),
      type: 'config',
      name: basename(path),
      filePath: path,
      summary: `Configuration file for the ${path.split('/')[1]} crate, defining ${tomlData.deps.length} dependencies and ${tomlData.features.length} features.`,
      tags: ['build-config', 'rust-crate', 'dependency-management'],
      complexity: 'simple'
    });

    // Config -> file edge
    edges.push({
      source: nodeId('config', path),
      target: nodeId('file', path),
      type: 'configures',
      direction: 'forward',
      weight: tomlData.deps.length
    });
  }

  // --- Test file: add tested_by edges to lib.rs ---
  if (isTest) {
    // Map test files to their target source file
    let libPath;
    if (path.includes('/tests/corpus.rs')) {
      libPath = 'crates/oxiguitar-edit/src/lib.rs';
    } else if (path.includes('/tests/entry.rs')) {
      libPath = 'crates/oxiguitar-edit/src/lib.rs';
    } else {
      libPath = path.replace('/tests/', '/src/');
    }
    // Only add edge if the target file exists in the batch
    if (inputPaths.has(libPath)) {
      edges.push({
        source: nodeId('file', path),
        target: nodeId('file', libPath),
        type: 'tested_by',
        direction: 'backward',
        weight: 1
      });
    }
  }

  // --- Functions from extraction results (deduplicated by name) ---
  const funcs = extractFile?.functions || [];
  const callGraph = extractFile?.callGraph || {};
  const callEdges = callGraph?.edges || [];

  // Deduplicate functions: if multiple share the same name in one file, keep first
  const seenFuncNames = new Set();
  const uniqueFuncs = [];
  for (const func of funcs) {
    if (!seenFuncNames.has(func.name)) {
      seenFuncNames.add(func.name);
      uniqueFuncs.push(func);
    }
  }

  for (const func of uniqueFuncs) {
    const funcId = nodeId('function', path, func.name);
    const cg = callEdges.filter(e => e.source === func.name);

    nodes.push({
      id: funcId,
      type: 'function',
      name: func.name,
      filePath: path,
      summary: funcSummary(func.name, cg.map(e => ({ target: e.target })), path),
      tags: [func.name.toLowerCase(), 'function', ...(file.tags || [])],
      complexity: complexity(func.bodyLines || 0, cg.length)
    });

    // File contains function
    edges.push({
      source: nodeId('file', path),
      target: funcId,
      type: 'contains',
      direction: 'forward',
      weight: 1
    });

    // Function call edges
    for (const ce of cg) {
      const targetId = nodeId('function', path, ce.target);
      edges.push({
        source: funcId,
        target: targetId,
        type: 'calls',
        direction: 'forward',
        weight: ce.weight || 1
      });
    }
  }

  // --- Structs from source parsing ---
  let parsed = null;
  if (!isConfig && !path.endsWith('.toml')) {
    parsed = parseRustSource(path, content);

    for (const s of parsed.structs) {
      const structId = nodeId('struct', path, s.name);
      nodes.push({
        id: structId,
        type: 'struct',
        name: s.name,
        filePath: path,
        summary: typeSummary(s.name, 'struct', path, s.fields),
        tags: [s.name.toLowerCase(), 'struct', ...(file.tags || [])],
        complexity: complexity(s.fields.length, 0)
      });
      edges.push({
        source: nodeId('file', path),
        target: structId,
        type: 'contains',
        direction: 'forward',
        weight: 1
      });
    }

    for (const e of parsed.enums) {
      const enumId = nodeId('enum', path, e.name);
      nodes.push({
        id: enumId,
        type: 'enum',
        name: e.name,
        filePath: path,
        summary: typeSummary(e.name, 'enum', path, e.variants),
        tags: [e.name.toLowerCase(), 'enum', ...(file.tags || [])],
        complexity: complexity(e.variants.length, 0)
      });
      edges.push({
        source: nodeId('file', path),
        target: enumId,
        type: 'contains',
        direction: 'forward',
        weight: 1
      });
    }

    for (const t of parsed.traits) {
      const traitId = nodeId('trait', path, t.name);
      nodes.push({
        id: traitId,
        type: 'trait',
        name: t.name,
        filePath: path,
        summary: typeSummary(t.name, 'trait', path),
        tags: [t.name.toLowerCase(), 'trait', ...(file.tags || [])],
        complexity: 'simple'
      });
      edges.push({
        source: nodeId('file', path),
        target: traitId,
        type: 'contains',
        direction: 'forward',
        weight: 1
      });
    }

    for (const m of parsed.modules) {
      const modId = nodeId('module', path, m.name);
      nodes.push({
        id: modId,
        type: 'module',
        name: m.name,
        filePath: path,
        summary: `Module \`${m.name}\` declared in ${basename(path)}.`,
        tags: [m.name.toLowerCase(), 'module', ...(file.tags || [])],
        complexity: 'simple'
      });
      edges.push({
        source: nodeId('file', path),
        target: modId,
        type: 'contains',
        direction: 'forward',
        weight: 1
      });
    }

    // --- Imports ---
    for (const imp of parsed.imports) {
      // Try to resolve to a known node
      const impTarget = nodeId('file', path, imp.split('::')[0]);
      // Create uses edge for imports
      // We'll create edges from file to import targets
      // For now, just note it
    }

    // --- Exports from extraction ---
    const exports = extractFile?.exports || [];
    for (const exp of exports) {
      const expName = typeof exp === 'string' ? exp : (exp.name || exp.id || exp.ident);
      // Try to find matching node: check struct/enum/trait FIRST, then function
      // This ensures exports of types target the correct node type
      const structId = nodeId('struct', path, expName);
      const enumId = nodeId('enum', path, expName);
      const traitId = nodeId('trait', path, expName);
      const funcId = nodeId('function', path, expName);

      // Check which node type actually exists
      let expId = null;
      // We need to check if the struct/enum/trait was actually created (they may not exist in nodes yet)
      // Since nodes are built incrementally, check if the type exists in our parsed data
      if (parsed) {
        if (parsed.structs.some(s => s.name === expName)) {
          expId = structId;
        } else if (parsed.enums.some(e => e.name === expName)) {
          expId = enumId;
        } else if (parsed.traits.some(t => t.name === expName)) {
          expId = traitId;
        } else {
          // Fall back to function
          expId = funcId;
        }
      } else {
        expId = funcId;
      }

      if (expId) {
        edges.push({
          source: nodeId('file', path),
          target: expId,
          type: 'exports',
          direction: 'forward',
          weight: 1
        });
      }
    }
  }
}

// --- Cross-file edges: lib.rs re-exports ---
// For oxiguitar-formats/src/lib.rs, add exports for each module
const formatsLibIdx = files.findIndex(f => f.path === 'crates/oxiguitar-formats/src/lib.rs');
if (formatsLibIdx >= 0) {
  const libPath = files[formatsLibIdx].path;
  const libContent = readFileSync(join(ROOT, libPath), 'utf8');
  const libParsed = parseRustSource(libPath, libContent);

  // Module-level exports (pub mod)
  for (const m of libParsed.modules) {
    // Find the module file
    const modFile = files.find(f => f.path === `${libPath.split('/').slice(0, 2).join('/')}/${m.name}` ||
                                     f.path === `${libPath.split('/').slice(0, 2).join('/')}/${libPath.split('/').slice(2).join('/')}/${m.name}`);
    if (modFile) {
      edges.push({
        source: nodeId('file', libPath),
        target: nodeId('file', modFile.path),
        type: 'exports',
        direction: 'forward',
        weight: 1
      });
    }
  }

  // pub use statements for re-exports
  const pubUseRegex = /pub\s+use\s+([^;]+);/g;
  let m2;
  while ((m2 = pubUseRegex.exec(libContent)) !== null) {
    const usePath = m2[1].trim();
    // Try to resolve to a target file
    const parts = usePath.split('::');
    if (parts.length >= 2) {
      const modName = parts[0];
      const targetFile = files.find(f => f.path.includes(modName) && f.path !== libPath);
      if (targetFile) {
        // Don't duplicate
        const exists = edges.some(e =>
          e.source === nodeId('file', libPath) && e.target === nodeId('file', targetFile.path) && e.type === 'exports'
        );
        if (!exists) {
          edges.push({
            source: nodeId('file', libPath),
            target: nodeId('file', targetFile.path),
            type: 'exports',
            direction: 'forward',
            weight: 1
          });
        }
      }
    }
  }
}

// --- Cross-file edges: oxiguitar-edit lib.rs ---
const editLibIdx = files.findIndex(f => f.path === 'crates/oxiguitar-edit/src/lib.rs');
if (editLibIdx >= 0) {
  const libPath = files[editLibIdx].path;
  const libContent = readFileSync(join(ROOT, libPath), 'utf8');
  const libParsed = parseRustSource(libPath, libContent);

  for (const m of libParsed.modules) {
    const modFile = files.find(f => f.path === `${libPath.split('/').slice(0, 2).join('/')}/${m.name}.rs` ||
                                     f.path === `${libPath.split('/').slice(0, 2).join('/')}/${libPath.split('/').slice(2).join('/')}/${m.name}`);
    if (modFile) {
      edges.push({
        source: nodeId('file', libPath),
        target: nodeId('file', modFile.path),
        type: 'exports',
        direction: 'forward',
        weight: 1
      });
    }
  }
}

// --- Write output ---
const output = {
  batchIndex: 16,
  nodes: nodes,
  edges: edges
};

writeFileSync(OUTPUT, JSON.stringify(output, null, 2), 'utf8');
console.log(`Batch 16: ${nodes.length} nodes, ${edges.length} edges`);
console.log('Node types:', [...new Set(nodes.map(n => n.type))].join(', '));
console.log('Edge types:', [...new Set(edges.map(e => e.type))].join(', '));
