import { readFileSync, writeFileSync } from 'fs';

const data = JSON.parse(readFileSync('/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-extract-results-21a.json', 'utf8'));

const nodes = [];
const edges = [];
const nodeIds = new Set();

function addNode(id, type, name, path, extra = {}) {
  if (nodeIds.has(id)) return;
  nodeIds.add(id);
  const node = {
    id,
    type,
    name,
    file: path,
  };
  Object.assign(node, extra);
  nodes.push(node);
}

function addEdge(source, target, relation, extra = {}) {
  const edge = { source, target, relation };
  Object.assign(edge, extra);
  edges.push(edge);
}

// Process each file result
for (const file of data.results) {
  const path = file.path;
  
  // Always add file-level node
  addNode(`file:${path}`, 'file', path, path);
  
  // Add struct/class nodes
  for (const cls of (file.classes || [])) {
    const structId = `struct:${path}:${cls.name}`;
    addNode(structId, 'struct', cls.name, path, {
      startLine: cls.startLine,
      endLine: cls.endLine,
      methods: cls.methods || [],
    });
    for (const method of (cls.methods || [])) {
      addEdge(structId, `function:${path}:${method}`, 'has_method');
    }
  }
  
  // Add enum nodes (from enums field if present)
  for (const enm of (file.enums || [])) {
    const enumId = `enum:${path}:${enm.name}`;
    addNode(enumId, 'enum', enm.name, path, {
      startLine: enm.startLine,
      endLine: enm.endLine,
      variants: enm.variants || [],
    });
    for (const variant of (enm.variants || [])) {
      addEdge(enumId, `variant:${path}:${enm.name}::${variant}`, 'has_variant');
    }
  }
  
  // Add trait nodes (from traits field if present)
  for (const trt of (file.traits || [])) {
    const traitId = `trait:${path}:${trt.name}`;
    addNode(traitId, 'trait', trt.name, path, {
      startLine: trt.startLine,
      endLine: trt.endLine,
      methods: trt.methods || [],
    });
    for (const method of (trt.methods || [])) {
      addEdge(traitId, `function:${path}:${method}`, 'has_method');
    }
  }
  
  // Add function nodes
  for (const func of (file.functions || [])) {
    const funcId = `function:${path}:${func.name}`;
    addNode(funcId, 'function', func.name, path, {
      startLine: func.startLine,
      endLine: func.endLine,
      params: func.params || [],
    });
  }
  
  // Add export nodes and edges
  for (const exp of (file.exports || [])) {
    const expId = `export:${path}:${exp.name}`;
    addNode(expId, 'export', exp.name, path, {
      line: exp.line,
      isDefault: exp.isDefault,
    });
    addEdge(expId, `function:${path}:${exp.name}`, 'exports');
  }
  
  // Add uses/imports
  for (const use of (file.uses || [])) {
    const useId = `use:${path}:${use.name}`;
    addNode(useId, 'use', use.name, path, {
      line: use.line,
      kind: use.kind || 'use',
    });
    addEdge(`file:${path}`, useId, 'imports');
  }
  
  // Add derives
  for (const derive of (file.derives || [])) {
    const deriveId = `derive:${path}:${derive.trait}`;
    addNode(deriveId, 'derive', derive.trait, path, {
      line: derive.line,
      target: derive.target,
    });
    if (derive.target) {
      addEdge(`struct:${path}:${derive.target}`, deriveId, 'derives');
    }
  }
  
  // Add macro nodes
  for (const macro of (file.macros || [])) {
    const macroId = `macro:${path}:${macro.name}`;
    addNode(macroId, 'macro', macro.name, path, {
      startLine: macro.startLine,
      endLine: macro.endLine,
    });
  }
  
  // Add module nodes
  for (const mod of (file.modules || [])) {
    const modId = `module:${path}:${mod.name}`;
    addNode(modId, 'module', mod.name, path, {
      startLine: mod.startLine,
      endLine: mod.endLine,
    });
    addEdge(`file:${path}`, modId, 'contains');
  }
  
  // Process call graph edges
  for (const call of (file.callGraph || [])) {
    const callerId = `function:${path}:${call.caller}`;
    const calleeId = call.callee;
    
    // Try to resolve callee to a function node
    if (calleeId.startsWith('self.') || calleeId.includes('::')) {
      // Cross-reference or method call
      const parts = calleeId.split('::');
      if (parts.length >= 2) {
        const targetModule = parts[0];
        const targetName = parts[parts.length - 1];
        const targetId = `function:${targetModule}:${targetName}`;
        addEdge(callerId, targetId, 'calls', { line: call.lineNumber });
      }
    }
    
    // For self.method calls, link to struct methods
    if (calleeId.startsWith('self.')) {
      const methodName = calleeId.replace('self.', '').split('.')[0];
      addEdge(callerId, `function:${path}:${methodName}`, 'calls_method', { line: call.lineNumber });
    }
  }
  
  // Add impl blocks
  for (const imp of (file.impls || [])) {
    const implId = `impl:${path}:${imp.trait || imp.type || 'impl'}`;
    addNode(implId, 'impl', imp.type || imp.trait || 'impl', path, {
      startLine: imp.startLine,
      endLine: imp.endLine,
      trait: imp.trait || null,
      for_type: imp.type || null,
      methods: imp.methods || [],
    });
    for (const method of (imp.methods || [])) {
      addEdge(implId, `function:${path}:${method}`, 'implements');
    }
    if (imp.type) {
      addEdge(`struct:${path}:${imp.type}`, implId, 'has_impl');
    }
  }
}

// Add cross-file dependencies based on import/uses patterns
// Look for common crate imports
for (const file of data.results) {
  const path = file.path;
  const allUses = file.uses || [];
  for (const use of allUses) {
    if (use.name.includes('oxiguitar_testgen') || use.name.includes('oxiguitar_tools')) {
      const targetPath = use.name.replace(/::/g, '/').replace(/.*src\//, 'crates/');
      if (targetPath !== path && nodeIds.has(`file:${targetPath}`)) {
        addEdge(`file:${path}`, `file:${targetPath}`, 'depends_on', { import: use.name });
      }
    }
  }
}

const output = { nodes, edges };
writeFileSync('/home/mat/Documents/src/oxiguitar/.ua/intermediate/batch-21a.json', JSON.stringify(output, null, 2));

console.log(`Generated ${nodes.length} nodes and ${edges.length} edges`);
console.log('Node types:', {});
const typeCounts = {};
nodes.forEach(n => { typeCounts[n.type] = (typeCounts[n.type] || 0) + 1; });
console.log(typeCounts);
const relCounts = {};
edges.forEach(e => { relCounts[e.relation] = (relCounts[e.relation] || 0) + 1; });
console.log('Edge relations:', relCounts);
