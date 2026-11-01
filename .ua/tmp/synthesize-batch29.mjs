import { readFileSync, writeFileSync } from 'fs';

const metadata = JSON.parse(readFileSync('.ua/tmp/batch29-extract/metadata.json', 'utf8'));
const input = JSON.parse(readFileSync('.ua/tmp/ua-file-analyzer-input-29.json', 'utf8'));

const nodes = [];
const edges = [];

// ---- Format nodes ----
nodes.push(
  { id: 'format:gp', type: 'format', name: 'Guitar Pro 7', description: 'ZIP-based format (GPX) with VERSION, Content/score.gpif, Content/PartConfiguration' },
  { id: 'format:gp5', type: 'format', name: 'Guitar Pro 5', description: 'Binary format with FICHIER GUITAR PRO v5.XX header' }
);
edges.push(
  { source: 'format:gp', relation: 'archive_type', target: 'zip' },
  { source: 'format:gp', relation: 'contains', target: 'gpif-content' },
  { source: 'format:gp5', relation: 'archive_type', target: 'binary' },
  { source: 'format:gp5', relation: 'header', target: 'FICHIER GUITAR PRO v5.XX' }
);

// ---- Build song metadata for each .gp file ----
function buildSongNodeGP(path, meta) {
  const base = path.replace(/\.gp$/, '');
  const id = `song:${base}.gp`;
  const node = { id, type: 'song' };

  if (meta.Title) node.title = meta.Title;
  if (meta.Artist) node.artist = meta.Artist;
  if (meta.Album) node.album = meta.Album;
  if (meta.Words) node.words = meta.Words;
  if (meta.Music) node.music = meta.Music;
  if (meta.WordsAndMusic) node.wordsAndMusic = meta.WordsAndMusic;
  if (meta.Copyright) node.copyright = meta.Copyright;
  if (meta.Tabber) node.tabber = meta.Tabber;
  if (meta.SubTitle) node.subTitle = meta.SubTitle;
  if (meta.Instructions) node.instructions = meta.Instructions;
  if (meta.Notices) node.notices = meta.Notices;

  if (meta.version === '7') {
    node.version = '7.0';
    node.notes = 'GP7 format (ZIP-based)';
  }
  if (meta.tempo) node.tempo = meta.tempo;
  if (meta.tempoDivision) node.tempoDivision = meta.tempoDivision;
  if (meta.tempoLabel) node.tempoLabel = meta.tempoLabel;
  if (meta.lyricsText) node.lyrics = meta.lyricsText;
  if (meta.markers && meta.markers.length) node.sections = meta.markers;

  // Track info
  if (meta.tracks && meta.tracks.length > 0) {
    node.trackCount = meta.tracks.length;
    if (meta.tracks.length === 1) {
      node.track0 = meta.tracks[0].name;
      if (meta.tracks[0].instrumentName) node.instrument0 = meta.tracks[0].instrumentName;
    } else {
      node.tracks = meta.tracks.map(t => t.name);
      if (meta.tracks[0].instrumentName) {
        meta.tracks.forEach((t, i) => {
          node[`instrument${i}`] = t.instrumentName;
        });
      }
    }
  }

  return node;
}

function buildSongNodeGP5(path, meta) {
  const base = path.replace(/\.gp5$/, '');
  const id = `song:${base}.gp5`;
  const node = { id, type: 'song' };

  if (meta.header) node.version = meta.header.replace('FICHIER GUITAR PRO v', '');
  if (meta.title) node.title = meta.title;
  if (meta.artist) node.artist = meta.artist;
  if (meta.album) node.album = meta.album;
  if (meta.instructions) node.instructions = meta.instructions;
  if (meta.notices) node.notices = meta.notices;
  if (meta.tabber) node.tabber = meta.tabber;
  if (meta.tempoLabel) node.tempoLabel = meta.tempoLabel;
  if (meta.lyricsText) node.lyrics = meta.lyricsText;

  if (meta.trackCount) node.trackCount = meta.trackCount;
  if (meta.track0) node.track0 = meta.track0;

  return node;
}

// ---- Process each .gp and .gp5 file ----
const gpEntries = Object.entries(metadata).filter(([p]) => p.endsWith('.gp') || p.endsWith('.gp5'));

for (const [path, meta] of gpEntries) {
  const isGP = path.endsWith('.gp');
  const base = path.replace(/\.(gp|gp5)$/, '');
  const format = isGP ? 'gp' : 'gp5';

  // file node
  const sizeLines = input.batchFiles.find(f => f.path === path)?.sizeLines ?? null;
  nodes.push({ id: `file:${path}`, type: 'file', format, language: format, sizeLines });

  // document node
  nodes.push({ id: `document:${path}`, type: 'document', format });

  // config node
  nodes.push({ id: `config:${path}`, type: 'config' });

  // song node
  const songNode = isGP ? buildSongNodeGP(path, meta) : buildSongNodeGP5(path, meta);
  nodes.push(songNode);

  // edges
  edges.push(
    { source: `file:${path}`, relation: 'contains', target: `document:${path}` },
    { source: `file:${path}`, relation: 'has_config', target: `config:${path}` },
    { source: `document:${path}`, relation: 'represents', target: songNode.id },
    { source: `config:${path}`, relation: 'specifies', target: songNode.id },
    { source: songNode.id, relation: 'uses_format', target: `format:${format}` }
  );
}

// ---- variant_of edges for .gp ↔ .gp5 pairs ----
// Pairs share the same base name (e.g., test-song-from-gp3.gp ↔ test-song-from-gp3.gp5)
const gpFiles = gpEntries.filter(([p]) => p.endsWith('.gp')).map(([p]) => p);
for (const gpPath of gpFiles) {
  const base = gpPath.replace(/\.gp$/, '');
  const gp5Path = `${base}.gp5`;
  if (metadata[gp5Path]) {
    const gpSongId = `song:${base}.gp`;
    const gp5SongId = `song:${base}.gp5`;
    edges.push(
      { source: gpSongId, relation: 'variant_of', target: gp5SongId },
      { source: gp5SongId, relation: 'variant_of', target: gpSongId }
    );
  }
}

// ---- .answer files ----
const answerFiles = input.batchFiles.filter(f => f.path.endsWith('.answer'));
for (const af of answerFiles) {
  nodes.push({ id: `file:${af.path}`, type: 'file', format: 'answer', language: 'answer', sizeLines: 0 });
  nodes.push({ id: `document:${af.path}`, type: 'document', format: 'answer' });
  nodes.push({ id: `config:${af.path}`, type: 'config' });

  const basename = af.path.split('/').pop();
  const songId = `song:${basename.replace('.answer', '')}`;
  nodes.push({
    id: songId,
    type: 'song',
    title: basename.replace('.answer', ''),
    instructions: `Guitar Pro plugin UI state file for ${basename}`,
    notes: 'Plugin UI state; stores widget configuration (clicks, score toggle, slider, field, choice, list, canvas)'
  });

  edges.push(
    { source: `file:${af.path}`, relation: 'contains', target: `document:${af.path}` },
    { source: `file:${af.path}`, relation: 'has_config', target: `config:${af.path}` },
    { source: `document:${af.path}`, relation: 'represents', target: songId },
    { source: `config:${af.path}`, relation: 'specifies', target: songId },
    { source: songId, relation: 'uses_format', target: 'format:answer' }
  );
}

// ---- format:answer node ----
nodes.push({
  id: 'format:answer',
  type: 'format',
  name: 'Guitar Pro Plugin Answer',
  description: 'Binary plugin UI state file storing widget configuration (clicks, toggle, slider, text field, choice, list, canvas)'
});

// ---- README.md ----
const readmePath = 'fixtures/reference/README.md';
const readmeContent = readFileSync(readmePath, 'utf8');
const lines = readmeContent.split('\n');

// Extract table rows for reference descriptions
const tableRows = [];
let inTable = false;
for (const line of lines) {
  if (line.startsWith('| ---')) { inTable = true; continue; }
  if (inTable && line.startsWith('| [`')) {
    const match = line.match(/\[`([^`]+)\.gp`\]\([^)]+\)\s*\|\s*(.+?)\s*\|/);
    if (match) {
      tableRows.push({ file: match[1], description: match[2].trim() });
    }
  }
}

nodes.push({ id: `file:${readmePath}`, type: 'file', format: 'markdown', language: 'markdown', sizeLines: 56 });
nodes.push({ id: `document:${readmePath}`, type: 'document', format: 'markdown' });

const readmeSongId = 'song:reference-readme';
nodes.push({
  id: readmeSongId,
  type: 'song',
  title: 'Visual-regression reference index',
  artist: 'oxiguitar-testgen',
  instructions: `Reference index for ${tableRows.length} visual-regression Guitar Pro 7 test files. Generated by \`oxiguitar-testgen\` (crates/oxiguitar-testgen/src/references.rs); checked by \`cargo test -p oxiguitar-testgen --test visual\`.`,
  notes: `Contains ${tableRows.length} reference entries covering: notes-and-rests, beams-and-flags, dots-and-ties, triplets, unusual-tuplets, chords-and-accidentals, key-signatures, time-signatures, repeats-and-endings, sections-and-directions, tempo-changes, two-voices, four-voices, rhythm-on-tab, standard-only, chord-names, beat-text, lyrics, dynamics, dynamics-hidden, grace-notes, hammer-on-pull-off, bends, slides, vibrato-and-trill, harmonics, dead-ghost-palm-mute, let-ring, articulations, tremolo-picking, strums-and-techniques, whammy-and-fade-in, effects-together, bass, seven-string, drop-d-and-capo, drums, piano-grand-staff, strings-treble-and-bass, notation-choice, header-and-credits, page-breaks, layout-vertical, layout-horizontal, layout-parchment, paper-letter, dark-page, tall-system-shrinks, eight-tracks, empty-bars`,
  referenceCount: tableRows.length,
  references: tableRows.map(r => ({ file: r.file, description: r.description }))
});

edges.push(
  { source: `file:${readmePath}`, relation: 'contains', target: `document:${readmePath}` },
  { source: `document:${readmePath}`, relation: 'describes', target: readmeSongId },
  { source: readmeSongId, relation: 'uses_format', target: 'format:markdown' }
);

// ---- format:markdown node ----
nodes.push({
  id: 'format:markdown',
  type: 'format',
  name: 'Markdown',
  description: 'Lightweight markup language for documentation'
});

// ---- Write output ----
const output = { nodes, edges };
writeFileSync('.ua/intermediate/batch-29.json', JSON.stringify(output, null, 2) + '\n', 'utf8');

console.log(`Generated ${nodes.length} nodes and ${edges.length} edges`);
console.log(`Files: ${gpEntries.length} .gp/.gp5 pairs, ${answerFiles.length} .answer files, 1 README`);
