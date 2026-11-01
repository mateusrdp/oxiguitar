import { execSync } from 'child_process';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { join, basename } from 'path';

const BASE = '/home/mat/Documents/src/oxiguitar';
const TMP = join(BASE, '.ua/tmp/batch29-extract');
mkdirSync(TMP, { recursive: true });

const gpFiles = [
  'fixtures/guitar-pro-check/test-song-from-gp3.gp',
  'fixtures/guitar-pro-check/test-song-from-gp4.gp',
  'fixtures/guitar-pro-check/test-song-from-gp5.gp',
  'fixtures/guitar-pro-check/test-song-from-gpx.gp',
  'fixtures/guitar-pro-check/time-signatures.gp',
  'fixtures/guitar-pro-check/tremolo-picking.gp',
  'fixtures/guitar-pro-check/triplets.gp',
  'fixtures/guitar-pro-check/two-voices.gp',
  'fixtures/guitar-pro-check/unusual-tuplets.gp',
  'fixtures/guitar-pro-check/vibrato-and-trill.gp',
  'fixtures/guitar-pro-check/whammy-and-fade-in.gp',
];

const gp5Files = [
  'fixtures/guitar-pro-check/test-song-from-gp3.gp5',
  'fixtures/guitar-pro-check/test-song-from-gp4.gp5',
  'fixtures/guitar-pro-check/test-song-from-gp5.gp5',
  'fixtures/guitar-pro-check/test-song-from-gpx.gp5',
  'fixtures/guitar-pro-check/time-signatures.gp5',
  'fixtures/guitar-pro-check/tremolo-picking.gp5',
  'fixtures/guitar-pro-check/triplets.gp5',
  'fixtures/guitar-pro-check/two-voices.gp5',
  'fixtures/guitar-pro-check/unusual-tuplets.gp5',
  'fixtures/guitar-pro-check/vibrato-and-trill.gp5',
  'fixtures/guitar-pro-check/whammy-and-fade-in.gp5',
];

function extractCdataValue(xml, tagName) {
  const match = xml.match(new RegExp(`<${tagName}><!\\[CDATA\\[(.*?)\\]\\]></${tagName}>`));
  return match ? match[1] : null;
}

function extractGpMetadata(relPath) {
  const dir = join(TMP, 'gp', join(relPath).replace(/[^a-zA-Z0-9]/g, '_'));
  mkdirSync(dir, { recursive: true });

  // Unzip
  execSync(`unzip -o "${join(BASE, relPath)}" -d "${dir}"`, { encoding: 'utf-8', stdio: 'pipe' });

  // Read VERSION file
  const versionPath = join(dir, 'VERSION');
  let gpVersion = null;
  if (existsSync(versionPath)) {
    gpVersion = readFileSync(versionPath, 'utf-8').trim();
  }

  // Read the score.gpif XML
  const scorePath = join(dir, 'Content', 'score.gpif');
  if (!existsSync(scorePath)) {
    return { error: 'No score.gpif found', relPath };
  }

  const content = readFileSync(scorePath, 'utf-8');

  // Parse GPVersion from root
  const gpVersionMatch = content.match(/<GPVersion>(\d+)<\/GPVersion>/);
  const version = gpVersionMatch ? gpVersionMatch[1] : (gpVersion || null);

  const result = { relPath, version };

  // Score metadata fields (inside <Score>...</Score>)
  const scoreFields = ['Title', 'SubTitle', 'Artist', 'Album', 'Words', 'Music',
                       'WordsAndMusic', 'Copyright', 'Tabber', 'Instructions', 'Notices'];
  for (const field of scoreFields) {
    const val = extractCdataValue(content, field);
    if (val) {
      result[field] = val;
    }
  }

  // Tempo from MasterTrack automations
  const tempoMatch = content.match(
    /<MasterTrack>[\s\S]*?<Automations>[\s\S]*?<Automation>[\s\S]*?<Type>Tempo<\/Type>[\s\S]*?<Value>(\d+)\s+(\d)<\/Value>/
  );
  if (tempoMatch) {
    result.tempo = parseInt(tempoMatch[1]);
    result.tempoDivision = parseInt(tempoMatch[2]);
  }

  // Tempo label (e.g., "Moderato") - look in lyrics or automation labels
  // In GP7, tempo labels appear in Lyrics section as text markers
  const tempoLabels = ['Moderato', 'Allegro', 'Andante', 'Adagio', 'Presto', 'Largo',
                       'Animato', 'Con fuoco', 'Dolce', 'Espressivo', 'Marcato', 'Maestoso',
                       'Morendo', 'Non troppo', 'Piano', 'Quasi', 'Ritardando', 'Secco',
                       'Senza sord', 'Sordino', 'Sostenuto', 'Staccato', 'Super'];
  for (const tl of tempoLabels) {
    const pattern = new RegExp(`<Text><!\\[CDATA\\[${tl}\\]\\]></Text>`);
    if (pattern.test(content)) {
      result.tempoLabel = tl;
      break;
    }
  }

  // Tracks
  const trackMatches = content.matchAll(/<Track id="(\d+)">([\s\S]*?)<\/Track>/g);
  const tracks = [];
  for (const m of trackMatches) {
    const trackId = m[1];
    const trackBlock = m[2];
    const trackName = extractCdataValue(trackBlock, 'Name');
    const shortName = extractCdataValue(trackBlock, 'ShortName');
    const instName = extractCdataValue(trackBlock, 'Name');
    const instType = extractCdataValue(trackBlock, 'Type');
    const lineCount = extractCdataValue(trackBlock, 'LineCount');

    // Instrument info is in <InstrumentSet>
    const instMatch = trackBlock.match(/<InstrumentSet>([\s\S]*?)<\/InstrumentSet>/);
    let instrumentName = null;
    let instrumentType = null;
    let lineCountNum = null;
    if (instMatch) {
      const instBlock = instMatch[1];
      instrumentName = extractCdataValue(instBlock, 'Name') ||
                       instBlock.match(/<Name>([^<]*)<\/Name>/)?.[1]?.trim();
      instrumentType = extractCdataValue(instBlock, 'Type') ||
                       instBlock.match(/<Type>([^<]*)<\/Type>/)?.[1]?.trim();
      lineCountNum = extractCdataValue(instBlock, 'LineCount') ||
                     instBlock.match(/<LineCount>([^<]*)<\/LineCount>/)?.[1]?.trim();
    }

    if (trackName) {
      tracks.push({
        id: trackId,
        name: trackName,
        shortName: shortName || null,
        instrumentName: instrumentName || null,
        instrumentType: instrumentType || null,
        lineCount: lineCountNum || null,
      });
    }
  }

  if (tracks.length > 0) {
    result.tracks = tracks;
    result.trackCount = tracks.length;
  }

  // Check for lyrics text (e.g., "la la la", "let it ring")
  const lyricsText = extractCdataValue(content, 'Words');
  if (lyricsText && lyricsText !== 'OxiGuitar') {
    result.lyricsText = lyricsText;
  }

  // Also check if there's lyrics text in the Lyrics section
  const lyricsSection = content.match(/<Lyrics[\s\S]*?dispatched="true"[^>]*>([\s\S]*?)<\/Lyrics>/);
  if (lyricsSection) {
    const lyricLines = [];
    const lineMatches = lyricsSection[1].matchAll(/<Text><!\[CDATA\[(.*?)\]\]><\/Text>/g);
    for (const lm of lineMatches) {
      if (lm[1].trim()) {
        lyricLines.push(lm[1].trim());
      }
    }
    if (lyricLines.length > 0 && !result.lyricsText) {
      result.lyricsText = lyricLines.join(' ');
    }
  }

  // Check for text/markers in the score (for tempo labels like "Moderato")
  const textMatches = content.matchAll(/<Text><!\[CDATA\[(.*?)\]\]><\/Text>/g);
  const allTexts = [];
  for (const tm of textMatches) {
    if (tm[1].trim() && !allTexts.includes(tm[1].trim())) {
      allTexts.push(tm[1].trim());
    }
  }
  if (allTexts.length > 0) {
    result.markers = allTexts;
  }

  return result;
}

function extractGp5Metadata(relPath) {
  const content = readFileSync(join(BASE, relPath), 'latin1');

  const result = { relPath };

  // Header
  const headerMatch = content.match(/(FICHIER GUITAR PRO v[\d.]+)/);
  if (headerMatch) result.header = headerMatch[1];

  // Extract all readable strings by splitting on null bytes and control sequences
  // GP5 uses length-prefixed strings separated by 01 00 00 00 00 or similar patterns
  // We'll use a more robust approach: split on null bytes, clean up, and filter
  const rawStrings = content.split(/\x00+/).map(s => s.trim()).filter(s => s.length > 1);

  // Known song titles for batch 29 feature files
  const knownTitles = [
    'OxiGuitar Test Song',
    'Time Signatures',
    'Tremolo Picking',
    'Triplets',
    'Two Voices',
    'Unusual Tuplets',
    'Vibrato and Trill',
    'Whammy and Fade In',
  ];

  // Known artist names
  const knownArtists = [
    'The OxiGuitar contributors',
    'OxiGuitar references',
  ];

  // Known instructions/patterns
  const knownInstructions = [
    "OxiGuitar test file: 4/4, 3/4, 6/8, 5/4, 7/8, 2/2 and 12/8 in a row.",
    "OxiGuitar test file: Tremolo picking at eighth, sixteenth and thirty-second speeds.",
    "OxiGuitar test file: Eighth, quarter and sixteenth triplets, and a triplet with a rest in it.",
    "OxiGuitar test file: Two voices on one staff: voice 1 stems up, voice 2 stems down, rests moved aside.",
    "OxiGuitar test file: Quintuplets, sextuplets, septuplets, a duplet and a 5:3 ratio (marked n:m).",
    "OxiGuitar test file: Vibrato on single notes and long notes, and trills with their other fret.",
    "OxiGuitar test file: A whammy-bar dive and a fade-in.",
  ];

  // Title: find the first known title in raw strings
  for (const s of rawStrings) {
    for (const title of knownTitles) {
      if (s.includes(title)) {
        result.title = title;
        break;
      }
    }
    if (result.title) break;
  }

  // Artist
  for (const s of rawStrings) {
    for (const artist of knownArtists) {
      if (s.includes(artist)) {
        result.artist = artist;
        break;
      }
    }
    if (result.artist) break;
  }

  // Album
  if (content.includes('Fixtures')) result.album = 'Fixtures';

  // Lyrics text
  if (content.includes('la la la')) result.lyricsText = 'la la la';
  if (content.includes('let it ring')) result.lyricsText = 'let it ring';

  // Tempo label
  const tempoLabels = ['Moderato', 'Allegro', 'Andante', 'Adagio', 'Presto', 'Largo'];
  for (const tl of tempoLabels) {
    if (content.includes(tl)) {
      result.tempoLabel = tl;
      break;
    }
  }

  // BPM - look for common BPM values in the binary
  // GP5 stores tempo as 2-byte big-endian integers
  const bpmMatches = content.matchAll(/\x00(\d{2,3})\x00/g);
  for (const m of bpmMatches) {
    const bpm = parseInt(m[1]);
    if (bpm > 30 && bpm < 300) {
      result.tempo = bpm;
      break;
    }
  }

  // Instructions
  for (const s of rawStrings) {
    for (const instr of knownInstructions) {
      if (s.includes(instr)) {
        result.instructions = instr;
        break;
      }
    }
    if (result.instructions) break;
  }

  // Tabber
  if (content.includes('OxiGuitar tests')) result.tabber = 'OxiGuitar tests';

  // Notices
  if (content.includes("Made for OxiGuitar's tests")) result.notices = "Made for OxiGuitar's tests";

  // Track names - find recognizable instrument names
  const trackNamePatterns = [
    'Lead Guitar', 'Rhythm Guitar', 'Acoustic Guitar', 'Bass Guitar',
    'Bass', 'Piano', 'Violin', 'Cello', 'Drums',
    'Guitar', 'Voice', 'Strings', 'Organ', 'Harp',
  ];
  const foundTracks = new Set();
  for (const name of trackNamePatterns) {
    // Use word boundary matching to avoid false positives
    const regex = new RegExp(`\\b${name}\\b`);
    if (regex.test(content)) {
      foundTracks.add(name);
    }
  }
  if (foundTracks.size > 0) {
    const trackArr = [...foundTracks];
    trackArr.forEach((t, i) => result[`track${i}`] = t);
    result.trackCount = trackArr.length;
  }

  return result;
}

// Process all files
const allResults = {};

console.log('=== Extracting .gp files ===');
for (const f of gpFiles) {
  console.log(`Processing: ${f}`);
  const result = extractGpMetadata(f);
  allResults[f] = result;
  console.log(`  ${JSON.stringify(result).substring(0, 300)}`);
}

console.log('\n=== Extracting .gp5 files ===');
for (const f of gp5Files) {
  console.log(`Processing: ${f}`);
  const result = extractGp5Metadata(f);
  allResults[f] = result;
  console.log(`  ${JSON.stringify(result).substring(0, 300)}`);
}

// Save results
writeFileSync(join(TMP, 'metadata.json'), JSON.stringify(allResults, null, 2));
console.log('\nSaved to', join(TMP, 'metadata.json'));
