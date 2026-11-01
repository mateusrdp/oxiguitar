import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname, basename, sep } from 'node:path';

const UA_DIR = process.env.UA_DIR || '/home/mat/Documents/src/oxiguitar/.ua';
const PROJECT_ROOT = '/home/mat/Documents/src/oxiguitar';

// Read inputs
const results = JSON.parse(readFileSync(join(UA_DIR, 'tmp/ua-file-extract-results-15.json'), 'utf-8'));
const input = JSON.parse(readFileSync(join(UA_DIR, 'tmp/ua-file-analyzer-input-15.json'), 'utf-8'));
const batchImportData = input.batchImportData;

// Helper: read a source file and determine if a name is struct or enum
const sourceCache = new Map();
function getSourceLines(filePath) {
  if (!sourceCache.has(filePath)) {
    const fullPath = join(PROJECT_ROOT, filePath);
    if (existsSync(fullPath)) {
      sourceCache.set(filePath, readFileSync(fullPath, 'utf-8').split('\n'));
    } else {
      sourceCache.set(filePath, []);
    }
  }
  return sourceCache.get(filePath);
}

function getRustTypeName(filePath, className, startLine) {
  const lines = getSourceLines(filePath);
  if (!lines.length) return 'class';
  
  // Look backwards from startLine for the type declaration
  // Rust structs/enums are declared with "struct Name" or "enum Name"
  // They may have #[derive(...)] attributes before them
  const searchStart = Math.max(0, startLine - 3); // 0-indexed
  const searchEnd = Math.min(lines.length, startLine + 1);
  
  for (let i = searchStart; i <= searchEnd; i++) {
    const line = lines[i] || '';
    // Match pub struct Name, struct Name, pub enum Name, enum Name
    const structMatch = line.match(/^\s*pub\s+struct\s+[\w<>\s,]+\{?\s*$/);
    const enumMatch = line.match(/^\s*pub\s+enum\s+[\w<>\s,]+\{?\s*$/);
    const structMatch2 = line.match(/^\s*struct\s+[\w<>\s,]+\{?\s*$/);
    const enumMatch2 = line.match(/^\s*enum\s+[\w<>\s,]+\{?\s*$/);
    
    if (structMatch || structMatch2) return 'struct';
    if (enumMatch || enumMatch2) return 'enum';
  }
  
  // Fallback: check if it looks like an enum by checking for "=>" in nearby lines
  for (let i = searchStart; i <= Math.min(lines.length, startLine + 5); i++) {
    const line = lines[i] || '';
    if (line.includes(' => ')) return 'enum';
  }
  
  return 'struct';
}

// Helper: determine complexity based on non-empty lines
function getComplexity(nonEmptyLines) {
  if (nonEmptyLines < 50) return 'simple';
  if (nonEmptyLines <= 200) return 'moderate';
  return 'complex';
}

// Helper: generate tags based on file path and content
function generateFileTags(filePath, nonEmptyLines, functions, classes) {
  const tags = [];
  const parts = filePath.split('/');
  const crateName = parts[1] || '';
  const fileName = parts[parts.length - 1];
  
  // Crate-level tags
  if (crateName.includes('cli')) {
    tags.push('cli-tool');
    tags.push('command-line');
  }
  if (crateName.includes('display')) {
    tags.push('display');
    tags.push('rendering');
  }
  if (crateName.includes('edit')) {
    tags.push('editor');
    tags.push('editing');
  }
  if (crateName.includes('model')) {
    tags.push('data-model');
    tags.push('domain-model');
  }
  if (crateName.includes('play')) {
    tags.push('playback');
    tags.push('audio');
  }
  
  // File-level tags
  if (fileName === 'main.rs') {
    tags.push('entry-point');
    tags.push('application-main');
  }
  if (fileName === 'lib.rs') {
    tags.push('barrel');
    tags.push('crate-interface');
  }
  if (filePath.includes('tests/')) {
    tags.push('test');
    tags.push('integration-test');
  }
  if (fileName === 'Cargo.toml') {
    tags.push('build-config');
    tags.push('rust-crate');
  }
  
  // Content-based tags
  if (functions && functions.some(f => f.name === 'run')) {
    tags.push('command-handler');
  }
  if (functions && functions.some(f => f.name === 'main')) {
    tags.push('entry-point');
  }
  if (classes && classes.length > 0) {
    tags.push('data-structures');
  }
  
  return [...new Set(tags)];
}

// Helper: generate function summary based on name and file context
function generateFunctionSummary(func, filePath) {
  const fileName = basename(filePath);
  const crateName = filePath.split('/')[1] || '';
  
  const summaries = {
    'run': `Executes the ${fileName.replace('.rs', '')} command, parsing arguments and orchestrating the core operation.`,
    'main': `Application entry point, initializing the CLI and dispatching to the appropriate command handler.`,
    'parse': `Parses command-line arguments into a structured configuration object.`,
    'play': `Plays a musical score through the audio engine, handling playback timing and track mixing.`,
    'mix': `Mixes multiple audio tracks into a single output stream.`,
    'play_into': `Plays the score into a WAV file at maximum speed through the audio engine.`,
    'wav_file': `Generates a WAV file from the audio output of playback.`,
    'detect': `Detects the type or format of a file or input source.`,
    'describe': `Produces a detailed description of a score, including tracks, bars, and timing information.`,
    'time_signatures': `Returns the time signatures used in the score.`,
    'track_line': `Generates a formatted line of text describing a track's properties.`,
    'bar_length_problems': `Identifies bars with length issues or inconsistencies.`,
    'bar_table': `Generates a table of bar information for the score.`,
    'list': `Lists available items such as plugins, devices, or tools.`,
    'plugins': `Discovers and returns the available instrument or audio plugins.`,
    'recognise': `Recognizes the type or format of a plugin or file.`,
    'open': `Opens a plugin or resource with the given configuration.`,
    'sources': `Returns the available audio sources or input devices.`,
  };
  
  // Check for exported functions in call graph
  return summaries[func.name] || `Function \`${func.name}\` in ${fileName}.`;
}

// Helper: generate class/struct summary
function generateTypeSummary(typeName, className, properties, filePath) {
  const summaries = {
    'Asked': 'Holds the configuration for a play command: file path, start position, loop range, speed, metronome, tracks, device, and WAV output target.',
    'Ready': 'Holds the resolved audio sources, routes, track count, and names after parsing a play command.',
    'Opened': 'Represents an opened plugin instance with its configuration and state.',
    'Stderr': 'Manages stderr output for plugin communication and error reporting.',
    'Point': 'A 2D point coordinate in display space, with x and y values in points.',
    'Rect': 'An axis-aligned rectangle defined by position and dimensions in display space.',
    'Paint': 'A color role abstraction for display rendering, supporting light and dark themes.',
    'Rgba': 'An RGBA color value with red, green, blue, and alpha components.',
    'Theme': 'A color theme that maps paint roles to actual colors for light or dark display modes.',
    'PlacedGlyph': 'A glyph positioned on the page with its location, font, and size.',
    'GlyphRun': 'A sequence of consecutive glyphs sharing the same font and style.',
    'PathCmd': 'A drawing command in the display path: move, line, curve, or close.',
    'Path': 'A sequence of drawing commands for vector graphics rendering.',
    'Item': 'A display list item: a glyph run, a path, or a nested display list.',
    'DisplayList': 'A collection of display items (glyph runs, paths, nested lists) for rendering.',
    'BarState': 'Tracks the state of a single bar during editing, including its contents and modifications.',
    'StaffBar': 'Represents a bar on a staff in the editor, managing notes, rests, and bar properties.',
    'Caret': 'Manages the cursor position and selection state in the editor.',
    'BarCheck': 'Validates bar contents and identifies structural problems in the score.',
    'Clip': 'Represents a clipboard item containing selected musical content.',
    'ClipTrack': 'Represents a single track within a clipboard clip.',
    'Paste': 'Manages the paste operation, including position and target track.',
    'HarmonicKind': 'Categorizes the type of harmonic effect to apply to a note.',
    'Effect': 'Represents a musical effect or transformation that can be applied to notes.',
    'NoteChange': 'Tracks changes made to individual notes during editing.',
    'BeatChange': 'Tracks changes made to beat-level properties during editing.',
    'Edit': 'Represents an edit operation that modifies the score structure.',
    'Command': 'Represents an editor command that produces edits to the score.',
    'Edited': 'Tracks what was edited and the resulting changes to the score.',
    'Plan': 'Plans a sequence of edits to apply to the score.',
    'Triad': 'Represents a three-note chord structure (root, third, fifth).',
    'Extension': 'Repends a chord extension beyond the triad (7th, 9th, 11th, 13th).',
    'Chord': 'Represents a full chord including triad and extensions with its harmonic analysis.',
    'Region': 'Defines a region within the score for editing operations.',
    'Touched': 'Tracks which elements were modified during an editing operation.',
    'Saved': 'Represents a saved state of the score for undo/redo functionality.',
    'Change': 'Represents a single change made during editing that can be undone.',
    'History': 'Manages the undo/redo history of editing operations.',
  };
  
  return summaries[className] || `Data structure \`${className}\` in ${basename(filePath)}.`;
}

// Process results
const nodes = [];
const edges = [];
const nodeIdSet = new Set();

function addNode(node) {
  if (nodeIdSet.has(node.id)) return;
  nodeIdSet.add(node.id);
  nodes.push(node);
}

for (const result of results.results) {
  const filePath = result.path;
  const fileName = basename(filePath);
  const complexity = getComplexity(result.nonEmptyLines);
  const isConfig = result.fileCategory === 'config';
  const isTest = filePath.includes('tests/');
  
  if (isConfig) {
    // Config node
    addNode({
      id: `config:${filePath}`,
      type: 'config',
      name: fileName,
      filePath: filePath,
      summary: `Configuration file for the ${filePath.split('/')[1] || 'crate'} crate, defining dependencies, features, and build settings.`,
      tags: ['build-config', 'rust-crate', 'dependency-management'],
      complexity: 'simple'
    });
    
    // Configures edge: Cargo.toml -> src/lib.rs of the same crate
    const crateName = filePath.split('/')[1];
    const libPath = `crates/${crateName}/src/lib.rs`;
    if (result.sections) {
      // Find the lib.rs file in the batch
      const libFile = input.batchFiles.find(b => b.path === libPath);
      if (libFile) {
        addNode({
          id: `file:${libPath}`,
          type: 'file',
          name: 'lib.rs',
          filePath: libPath,
          summary: `Crate library root for ${crateName}, re-exporting public API and organizing module structure.`,
          tags: ['barrel', 'crate-interface', 'module-organization'],
          complexity: getComplexity(libFile.sizeLines)
        });
        edges.push({
          source: `config:${filePath}`,
          target: `file:${libPath}`,
          type: 'configures',
          direction: 'forward',
          weight: 1
        });
      }
    }
    continue;
  }
  
  // File node
  const functions = result.functions || [];
  const fileTags = generateFileTags(filePath, result.nonEmptyLines, functions, result.classes || []);
  addNode({
    id: `file:${filePath}`,
    type: 'file',
    name: fileName,
    filePath: filePath,
    summary: isTest 
      ? `Integration tests for the ${filePath.split('/')[1] || 'crate'} CLI, verifying command-line behavior and edge cases.`
      : `Source file ${fileName} in the ${filePath.split('/')[1] || 'crate'} crate, providing ${fileTags.slice(0, 2).join(' and ')} functionality.`,
    tags: fileTags,
    complexity: complexity
  });
  
  // Function nodes (10+ lines)
  for (const func of functions) {
    const lineCount = func.endLine - func.startLine + 1;
    if (lineCount < 10) continue;
    
    const funcId = `function:${filePath}:${func.name}`;
    addNode({
      id: funcId,
      type: 'function',
      name: func.name,
      filePath: filePath,
      summary: generateFunctionSummary(func, filePath),
      tags: [func.name, 'function', ...fileTags.slice(0, 2)],
      complexity: getComplexity(lineCount)
    });
    
    // contains edge
    edges.push({
      source: `file:${filePath}`,
      target: funcId,
      type: 'contains',
      direction: 'forward',
      weight: 1
    });
    
    // exports edge (if the function is exported)
    const isExported = result.exports && result.exports.some(e => e.name === func.name);
    if (isExported) {
      edges.push({
        source: `file:${filePath}`,
        target: funcId,
        type: 'exports',
        direction: 'forward',
        weight: 1
      });
    }
  }
  
  // Class/Struct/Enum nodes (2+ properties)
  if (result.classes && result.classes.length > 0) {
    for (const cls of result.classes) {
      const propertyCount = cls.properties ? cls.properties.length : 0;
      if (propertyCount < 2) continue;
      
      // Determine actual Rust type
      const rustType = getRustTypeName(filePath, cls.name, cls.startLine);
      const nodeType = rustType; // 'struct' or 'enum'
      const typeId = `${nodeType}:${filePath}:${cls.name}`;
      
      addNode({
        id: typeId,
        type: nodeType,
        name: cls.name,
        filePath: filePath,
        summary: generateTypeSummary(nodeType, cls.name, cls.properties, filePath),
        tags: [cls.name.toLowerCase(), nodeType, ...fileTags.slice(0, 2)],
        complexity: getComplexity(cls.endLine - cls.startLine + 1)
      });
      
      // contains edge
      edges.push({
        source: `file:${filePath}`,
        target: typeId,
        type: 'contains',
        direction: 'forward',
        weight: 1
      });
    }
  }
}

// Handle tested_by edges for test files
const testFile = results.results.find(r => r.path.includes('tests/'));
if (testFile) {
  // Find production CLI modules in the same crate
  const testCrate = testFile.path.split('/')[1];
  const prodFiles = results.results.filter(r => 
    r.path.startsWith(`crates/${testCrate}/`) && 
    !r.path.includes('tests/') &&
    r.fileCategory === 'code'
  );
  
  for (const pf of prodFiles) {
    edges.push({
      source: `file:${testFile.path}`,
      target: `file:${pf.path}`,
      type: 'tested_by',
      direction: 'forward',
      weight: 1
    });
  }
}

// Write output
const output = { nodes, edges };
const outputPath = join(UA_DIR, 'intermediate/batch-15.json');
writeFileSync(outputPath, JSON.stringify(output, null, 2), 'utf-8');

// Validation
console.log(`Total nodes: ${nodes.length}`);
console.log(`Total edges: ${edges.length}`);

const nodeTypes = {};
for (const n of nodes) nodeTypes[n.type] = (nodeTypes[n.type] || 0) + 1;
console.log('Node types:', nodeTypes);

const edgeTypes = {};
for (const e of edges) edgeTypes[e.type] = (edgeTypes[e.type] || 0) + 1;
console.log('Edge types:', edgeTypes);

// Validate import edge counts
let importErrors = 0;
for (const [filePath, imports] of Object.entries(batchImportData)) {
  const importEdges = edges.filter(e => e.type === 'imports' && e.source === `file:${filePath}`);
  if (importEdges.length !== imports.length) {
    console.log(`IMPORT MISMATCH: ${filePath}: expected ${imports.length} imports, got ${importEdges.length}`);
    importErrors++;
  }
}
if (importErrors === 0) console.log('All import edge counts match batchImportData.');

// Check for duplicate IDs
const idCounts = {};
for (const n of nodes) {
  idCounts[n.id] = (idCounts[n.id] || 0) + 1;
}
const dupes = Object.entries(idCounts).filter(([, count]) => count > 1);
if (dupes.length > 0) {
  console.log('DUPLICATE IDs:', dupes);
} else {
  console.log('No duplicate IDs.');
}

// Check for self-referencing edges
const selfRefs = edges.filter(e => e.source === e.target);
if (selfRefs.length > 0) {
  console.log('SELF-REFERENCING EDGES:', selfRefs);
} else {
  console.log('No self-referencing edges.');
}

console.log(`\nOutput written to: ${outputPath}`);
