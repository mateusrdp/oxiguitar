import { readFileSync, writeFileSync } from "fs";

const input = JSON.parse(readFileSync("/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-extract-results-18.json", "utf-8"));
const batchInput = JSON.parse(readFileSync("/home/mat/Documents/src/oxiguitar/.ua/tmp/ua-file-analyzer-input-18.json", "utf-8"));

const nodes = [];
const edges = [];
const nodeIds = new Set();

// Type definitions identified from reading the source files
const types = {
  "crates/oxiguitar-layout/src/lib.rs": [
    { name: "Layout", type: "enum", summary: "Enum of supported layout styles (standard tab, grand staff, etc.) with from_name constructor.", tags: ["layout", "enum", "engraving"] },
    { name: "Paper", type: "struct", summary: "Paper size and page dimensions for engraving output.", tags: ["layout", "struct", "paper"] },
    { name: "Options", type: "struct", summary: "Configuration options for the engraving process.", tags: ["layout", "struct", "configuration"] },
    { name: "Notation", type: "struct", summary: "Notation-specific settings (clef, key signature, etc.).", tags: ["layout", "struct", "notation"] },
    { name: "Engraved", type: "struct", summary: "Result of the engraving process, containing drawn systems and layout data.", tags: ["layout", "struct", "engraving", "output"] },
  ],
  "crates/oxiguitar-layout/src/engrave.rs": [
    { name: "Lane", type: "struct", summary: "A vertical lane in the tab staff for a single string, managing glyph placement.", tags: ["layout", "struct", "lane", "tab"] },
    { name: "Band", type: "struct", summary: "A horizontal band spanning multiple lanes, managing vertical glyph placement.", tags: ["layout", "struct", "band", "layout"] },
    { name: "DrawnSystem", type: "struct", summary: "An engraved musical system (row of bars) with all drawn elements.", tags: ["layout", "struct", "engraving", "system"] },
    { name: "Stave", type: "struct", summary: "A single staff (tab or standard notation) within a system.", tags: ["layout", "struct", "engraving", "staff"] },
  ],
  "crates/oxiguitar-layout/src/plan.rs": [
    { name: "NoteInfo", type: "struct", summary: "Information about a single note for layout planning (position, duration, etc.).", tags: ["layout", "struct", "planning", "note"] },
    { name: "BeatInfo", type: "struct", summary: "Information about a beat for layout planning.", tags: ["layout", "struct", "planning", "beat"] },
    { name: "BarPlan", type: "struct", summary: "Layout plan for a single bar, including lanes, bands, and spacing.", tags: ["layout", "struct", "planning", "bar"] },
    { name: "Planner", type: "struct", summary: "Planner that computes bar layouts from score data.", tags: ["layout", "struct", "planning"] },
  ],
  "crates/oxiguitar-layout/src/system.rs": [
    { name: "View", type: "struct", summary: "A view of the score for system layout (track selection, etc.).", tags: ["layout", "struct", "system", "view"] },
    { name: "SystemSpec", type: "struct", summary: "Specification for a system including bar ranges and layout parameters.", tags: ["layout", "struct", "system", "specification"] },
  ],
  "crates/oxiguitar-layout/src/hits.rs": [
    { name: "Hits", type: "struct", summary: "Container for click-to-score hit regions across all bar types.", tags: ["layout", "struct", "hits", "interaction"] },
    { name: "SystemHits", type: "struct", summary: "Hit regions for a system, aggregating bar-level hits.", tags: ["layout", "struct", "hits", "system"] },
    { name: "StaveHits", type: "struct", summary: "Hit regions for a single staff within a bar.", tags: ["layout", "struct", "hits", "staff"] },
    { name: "BarHits", type: "struct", summary: "Hit regions for a single bar.", tags: ["layout", "struct", "hits", "bar"] },
    { name: "BeatHits", type: "struct", summary: "Hit regions for a single beat.", tags: ["layout", "struct", "hits", "beat"] },
    { name: "TabHits", type: "struct", summary: "Hit regions for tab notation elements.", tags: ["layout", "struct", "hits", "tab"] },
  ],
  "crates/oxiguitar-layout/src/draw.rs": [
    { name: "G", type: "struct", summary: "SMuFL glyph identifier for music notation rendering.", tags: ["layout", "struct", "glyph", "smufl"] },
    { name: "Painter", type: "struct", summary: "Drawing context providing methods for rendering glyphs, text, and lines.", tags: ["layout", "struct", "drawing", "painter"] },
  ],
  "crates/oxiguitar-layout/src/pitch.rs": [
    { name: "Spelled", type: "struct", summary: "A written note with accidental information (spelling).", tags: ["layout", "struct", "pitch", "accidental"] },
    { name: "BarAccidentals", type: "struct", summary: "Accidental information computed for a bar.", tags: ["layout", "struct", "pitch", "accidental", "bar"] },
  ],
  "crates/oxiguitar-layout/src/grid.rs": [
    { name: "GridCol", type: "struct", summary: "A grid column representing an onset position in the rhythm grid.", tags: ["layout", "struct", "grid", "rhythm"] },
    { name: "BarGrid", type: "struct", summary: "Grid mapping onsets to column positions for precise layout.", tags: ["layout", "struct", "grid"] },
  ],
  "crates/oxiguitar-layout/src/rhythm.rs": [
    { name: "BeatRef", type: "struct", summary: "Reference to a beat with position information.", tags: ["layout", "struct", "rhythm", "beat"] },
    { name: "Column", type: "struct", summary: "A rhythm column representing a point in time within a beat.", tags: ["layout", "struct", "rhythm", "column"] },
  ],
  "crates/oxiguitar-layout/src/page.rs": [
    { name: "Header", type: "struct", summary: "Page header containing title, composer, and other metadata.", tags: ["layout", "struct", "page", "header"] },
  ],
  "crates/oxiguitar-model/src/score.rs": [
    { name: "Score", type: "struct", summary: "Top-level score container holding tracks, metadata, and global settings.", tags: ["model", "struct", "score"] },
    { name: "MasterBar", type: "struct", summary: "Master bar containing time and key signature changes.", tags: ["model", "struct", "bar", "master"] },
    { name: "TimeSignature", type: "struct", summary: "Time signature definition (numerator/denominator).", tags: ["model", "struct", "time", "signature"] },
    { name: "KeySignature", type: "struct", summary: "Key signature with fifths and mode information.", tags: ["model", "struct", "key", "signature"] },
  ],
  "crates/oxiguitar-model/src/track.rs": [
    { name: "Track", type: "struct", summary: "A musical track containing bars, tuning, and mix information.", tags: ["model", "struct", "track"] },
    { name: "Staff", type: "struct", summary: "A staff within a track (may have multiple for keyboard).", tags: ["model", "struct", "staff"] },
    { name: "Instrument", type: "struct", summary: "Instrument definition with name and properties.", tags: ["model", "struct", "instrument"] },
    { name: "Mix", type: "struct", summary: "Mix settings for a track (pan, volume).", tags: ["model", "struct", "mix"] },
    { name: "ChordDiagram", type: "struct", summary: "Chord diagram definition for display.", tags: ["model", "struct", "chord", "diagram"] },
  ],
  "crates/oxiguitar-model/src/bar.rs": [
    { name: "Bar", type: "struct", summary: "A musical bar containing voices, beats, and bar-line information.", tags: ["model", "struct", "bar"] },
    { name: "Voice", type: "struct", summary: "A voice within a bar containing beats.", tags: ["model", "struct", "voice"] },
    { name: "Beat", type: "struct", summary: "A beat within a voice containing notes and effects.", tags: ["model", "struct", "beat"] },
    { name: "BeatEffects", type: "struct", summary: "Effects applied at the beat level (bends, slides, etc.).", tags: ["model", "struct", "effects", "beat"] },
    { name: "Clef", type: "enum", summary: "Enum of supported clef types for notation.", tags: ["model", "enum", "clef"] },
    { name: "BeatKind", type: "enum", summary: "Enum of beat types (normal, grace, etc.).", tags: ["model", "enum", "beat", "kind"] },
  ],
  "crates/oxiguitar-model/src/note.rs": [
    { name: "Note", type: "struct", summary: "A single musical note with pitch, duration, and effects.", tags: ["model", "struct", "note"] },
    { name: "NoteEffects", type: "struct", summary: "Effects applied to a single note (bend, vibrato, etc.).", tags: ["model", "struct", "effects", "note"] },
    { name: "NotePlace", type: "enum", summary: "Enum describing where a note is placed (fret, harmonic, etc.).", tags: ["model", "enum", "note", "place"] },
    { name: "Pitch", type: "struct", summary: "Pitch value with octave and semitone information.", tags: ["model", "struct", "pitch"] },
    { name: "Bend", type: "struct", summary: "Bend definition with start pitch, target, and type.", tags: ["model", "struct", "bend"] },
    { name: "Harmonic", type: "struct", summary: "Harmonic type (natural, artificial, tapped).", tags: ["model", "struct", "harmonic"] },
  ],
  "crates/oxiguitar-model/src/duration.rs": [
    { name: "Duration", type: "struct", summary: "Note duration with visual and rhythmic representation.", tags: ["model", "struct", "duration"] },
    { name: "NoteValue", type: "enum", summary: "Enum of note values (whole, half, quarter, eighth, etc.).", tags: ["model", "enum", "duration", "note-value"] },
    { name: "Tuplet", type: "struct", summary: "Tuplet definition (e.g., triplet, quintuplet).", tags: ["model", "struct", "tuplet"] },
  ],
  "crates/oxiguitar-model/src/ratio.rs": [
    { name: "Ratio", type: "struct", summary: "Exact rational number type for precise timing calculations.", tags: ["model", "struct", "ratio", "math"] },
  ],
  "crates/oxiguitar-model/src/tuning.rs": [
    { name: "Pattern", type: "struct", summary: "Tuning pattern defining string pitches and recognition rules.", tags: ["model", "struct", "tuning", "pattern"] },
  ],
};

// Helper: check if a callee is a builtin/stdlib method to skip
function isBuiltinCallee(callee) {
  const low = callee.toLowerCase();
  const skipPatterns = [
    "self.", "map(", "iter(", "collect(", "unwrap_or", "get(", "copied(",
    "filter(", "len(", "push(", "clone(", "default(", "new(", "from_",
    "into_", "unwrap(", "expect(", "ok(", "is_", "has_", "as_", "to_",
    "with_", "for_", "or_", "and_", "not_", "size_of", "width", "height",
    "path", "env!", "join(", "display", "DisplayList", "Path::",
    "String::", "Vec::", "HashMap::", "Option::", "Result::", "Some(",
    "None", "Ok(", "Err(", "Debug", "Display", "Default", "Clone",
    "Copy", "PartialEq", "Eq", "Ord", "PartialOrd", "Hash", "From<",
    "Into<", "IntoIterator", "Iterator", "Extend", "Index", "Deref",
    "Borrow", "AsRef", "ToString", "fmt(", "write!", "writeln!",
    "format!", "println!", "eprintln!", "panic!", "unreachable!",
    "todo!", "unimplemented!", "assert!", "debug_assert!", "cfg!",
    "allow!", "warn!", "deny!", "forbid!", "macro_rules!", "include!",
    "concat!", "file!(", "line!(", "column!(", "module_path!(",
    "option_env!", "min(", "max(", "abs(", "rem_euclid(", "div(",
    "mul(", "add(", "sub(", "neg(", "from_i128", "from_i32", "from_i16",
    "from_u32", "from_u16", "from_u8", "try_from", "try_into",
    "floor(", "ceil(", "round(", "trunc(", "fract(", "is_nan",
    "is_infinite", "is_finite", "is_normal", "is_subnormal",
    "to_bits", "to_le_bits", "to_be_bits", "from_bits", "from_le_bits",
    "from_be_bits", "to_le", "to_be", "from_le", "from_be",
    "swap_bytes", "checked_", "wrapping_", "saturating_", "overflowing_",
    "rotate_left", "rotate_right", "count_ones", "count_zeros",
    "leading_zeros", "trailing_zeros", "leading_ones", "trailing_ones",
    "reverse(", "pow(", "sqrt(", "cbrt(", "exp(", "exp2(", "ln(",
    "log(", "log2(", "log10(", "sin(", "cos(", "tan(", "asin(",
    "acos(", "atan(", "sinh(", "cosh(", "tanh(", "asinh(", "acosh(",
    "atanh(", "hypot(", "atan2(", "to_degrees", "to_radians",
    "to_string(", "as_str", "as_mut", "as_ref", "unwrap_or_default",
    "unwrap_or_else", "ok_or", "ok_or_else", "and_then", "or_else",
    "map_or", "map_or_else", "transpose", "unzip", "position",
    "rposition", "all(", "any(", "find(", "enumerate(", "zip(",
    "rev(", "skip(", "take(", "chain(", "flat_map(", "filter_map(",
    "fold(", "try_fold(", "for_each(", "reduce(", "next(", "peek(",
    "size_hint", "nth(", "last(", "nth_back", "count(", "sum(",
    "product", "eq(", "ne(", "partial_cmp", "cmp", "bail", "anyhow",
    "io::", "std::", "core::", "alloc::", "f64::", "f32::",
    "i32::", "i64::", "u32::", "u64::", "usize::", "bool::",
    "char::", "str::", "slice::", "array::", "box::", "rc::",
    "arc::", "cell::", "sync::", "io::Error", "io::Result",
    "io::stdin", "io::stdout", "io::stderr", "io::Read", "io::Write",
    "fmt::Display", "fmt::Debug", "fmt::Formatter", "fmt::Result",
    "path::Path", "path::PathBuf", "fs::", "time::", "net::",
    "thread::", "mem::", "ops::", "convert::", "marker::", "borrow::",
    "cmp::", "iter::", "result::", "option::", "string::", "vec::Vec",
    "collections::", "boxed::Box", "rc::Rc", "sync::Arc",
    "cell::Cell", "cell::RefCell", "sync::Mutex", "sync::RwLock",
    "sync::atomic", "sync::Barrier", "sync::Condvar", "sync::Once",
    "sync::OnceLock", "sync::LazyLock", "task::", "pin::Pin",
    "future::Future", "poll::Poll", "task::Context", "task::Waker",
    "mem::forget", "mem::swap", "mem::replace", "mem::take",
    "mem::size_of", "mem::align_of", "mem::MaybeUninit", "mem::drop",
    "ptr::", "ptr::null", "ptr::null_mut", "ptr::read", "ptr::write",
    "ptr::copy", "ptr::copy_nonoverlapping", "ptr::read_volatile",
    "ptr::write_volatile", "ptr::eq", "ptr::ne", "ptr::addr_of",
    "ptr::addr_of_mut", "intrinsics::", "simd::", "simd::Simd",
    "intrinsics::assume", "intrinsics::assert_inhabited",
    "intrinsics::assert_zst", "intrinsics::assert_mem_valid",
    "intrinsics::exact_div", "intrinsics::forget", "intrinsics::likely",
  ];
  for (const p of skipPatterns) {
    if (low.includes(p)) return true;
  }
  return false;
}

// 1. Build file nodes
for (const file of batchInput.batchFiles) {
  const id = `file:${file.path}`;
  if (nodeIds.has(id)) continue;
  nodeIds.add(id);

  let nodeType = "file";
  let tags = [];
  let summary = "";
  let complexity = "simple";

  if (file.path.endsWith(".toml")) {
    nodeType = "config";
    tags = ["config", "cargo", "dependency"];
    const crateName = file.path.includes("oxiguitar-layout") ? "oxiguitar-layout" :
                      file.path.includes("oxiguitar-model") ? "oxiguitar-model" : "oxiguitar-play";
    summary = `Cargo configuration file for ${crateName} crate.`;
  } else if (file.path.includes("oxiguitar-layout")) {
    tags = ["layout", "engraving", "guitar-tablature", "sheet-music"];
    summary = `Source file ${file.path.split("/").pop()} in the oxiguitar-layout crate, providing engraving and layout functionality for guitar tablature.`;
    complexity = file.path.includes("engrave.rs") || file.path.includes("effects.rs") ? "complex" :
                 file.path.includes("plan.rs") || file.path.includes("hits.rs") ? "moderate" : "simple";
  } else if (file.path.includes("oxiguitar-model")) {
    tags = ["model", "data-structure", "guitar-tablature", "music-theory"];
    summary = `Source file ${file.path.split("/").pop()} in the oxiguitar-model crate, providing core data structures for guitar tablature representation.`;
    complexity = file.path.includes("score.rs") || file.path.includes("wire.rs") ? "complex" :
                 file.path.includes("bar.rs") || file.path.includes("note.rs") ? "moderate" : "simple";
  } else {
    tags = ["unknown"];
    summary = `File ${file.path}.`;
  }

  nodes.push({
    id, type: nodeType, name: file.path.split("/").pop(),
    filePath: file.path, summary, tags, complexity,
  });
}

// 2. Build function nodes
for (const result of input.results) {
  const path = result.path;
  const funcs = result.functions || [];

  for (const fn of funcs) {
    const id = `function:${path}:${fn.name}`;
    if (nodeIds.has(id)) continue;
    nodeIds.add(id);

    const paramStr = fn.params.length > 0
      ? `accepting ${fn.params.length} parameter(s): ${fn.params.join(", ")}`
      : "with no parameters";
    const summary = `Function ${fn.name} in ${path.split("/").pop()}, ${paramStr} (lines ${fn.startLine}-${fn.endLine}).`;

    nodes.push({
      id, type: "function", name: fn.name, filePath: path,
      summary, tags: [fn.name, "function"], complexity: "simple",
    });

    edges.push({
      source: `file:${path}`, target: id, type: "contains",
      direction: "forward", weight: 1,
    });
  }
}

// 3. Build type nodes
for (const [filePath, typeList] of Object.entries(types)) {
  for (const t of typeList) {
    const id = `${t.type}:${filePath}:${t.name}`;
    if (nodeIds.has(id)) continue;
    nodeIds.add(id);

    nodes.push({
      id, type: t.type, name: t.name, filePath,
      summary: t.summary, tags: t.tags, complexity: "moderate",
    });

    edges.push({
      source: `file:${filePath}`, target: id, type: "contains",
      direction: "forward", weight: 1,
    });
  }
}

// 4. Build module nodes
const moduleMap = {
  "crates/oxiguitar-layout/src/lib.rs": ["draw", "effects", "engrave", "grid", "hands", "hits", "page", "pitch", "plan", "rhythm", "system", "glyphs"],
  "crates/oxiguitar-model/src/lib.rs": ["bar", "duration", "lyrics", "note", "ratio", "score", "track", "tuning", "wire"],
};

for (const [filePath, modules] of Object.entries(moduleMap)) {
  for (const mod of modules) {
    const id = `module:${filePath}:${mod}`;
    if (nodeIds.has(id)) continue;
    nodeIds.add(id);

    nodes.push({
      id, type: "module", name: mod, filePath,
      summary: `Module ${mod} in ${filePath.split("/").pop()}.`,
      tags: [mod, "module"], complexity: "simple",
    });

    edges.push({
      source: `file:${filePath}`, target: id, type: "contains",
      direction: "forward", weight: 1,
    });
  }
}

// 5. Build call edges from callGraph
for (const result of input.results) {
  const path = result.path;
  const callGraph = result.callGraph || [];

  for (const call of callGraph) {
    const caller = call.caller;
    const callee = call.callee;
    if (!caller || !callee) continue;

    // Skip builtin/stdlib calls
    if (isBuiltinCallee(callee)) continue;

    const callerId = `function:${path}:${caller}`;
    const calleeId = `function:${path}:${callee}`;

    if (nodeIds.has(callerId) && nodeIds.has(calleeId) && callerId !== calleeId) {
      edges.push({
        source: callerId, target: calleeId, type: "calls",
        direction: "forward", weight: 1,
      });
    }
  }
}

// 6. Cross-file import edges
const crossFileImports = [
  // layout -> model
  ["crates/oxiguitar-layout/src/lib.rs", "crates/oxiguitar-model/src/score.rs"],
  ["crates/oxiguitar-layout/src/lib.rs", "crates/oxiguitar-model/src/track.rs"],
  ["crates/oxiguitar-layout/src/lib.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-layout/src/lib.rs", "crates/oxiguitar-model/src/note.rs"],
  // layout internal
  ["crates/oxiguitar-layout/src/engrave.rs", "crates/oxiguitar-layout/src/plan.rs"],
  ["crates/oxiguitar-layout/src/engrave.rs", "crates/oxiguitar-layout/src/grid.rs"],
  ["crates/oxiguitar-layout/src/engrave.rs", "crates/oxiguitar-layout/src/pitch.rs"],
  ["crates/oxiguitar-layout/src/engrave.rs", "crates/oxiguitar-layout/src/rhythm.rs"],
  ["crates/oxiguitar-layout/src/engrave.rs", "crates/oxiguitar-layout/src/hits.rs"],
  ["crates/oxiguitar-layout/src/plan.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-layout/src/plan.rs", "crates/oxiguitar-model/src/note.rs"],
  ["crates/oxiguitar-layout/src/plan.rs", "crates/oxiguitar-model/src/duration.rs"],
  ["crates/oxiguitar-layout/src/system.rs", "crates/oxiguitar-layout/src/plan.rs"],
  ["crates/oxiguitar-layout/src/system.rs", "crates/oxiguitar-layout/src/hits.rs"],
  ["crates/oxiguitar-layout/src/hits.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-layout/src/hits.rs", "crates/oxiguitar-model/src/note.rs"],
  ["crates/oxiguitar-layout/src/pitch.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-layout/src/pitch.rs", "crates/oxiguitar-model/src/note.rs"],
  ["crates/oxiguitar-layout/src/grid.rs", "crates/oxiguitar-layout/src/plan.rs"],
  ["crates/oxiguitar-layout/src/rhythm.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-layout/src/rhythm.rs", "crates/oxiguitar-model/src/note.rs"],
  ["crates/oxiguitar-layout/src/hands.rs", "crates/oxiguitar-model/src/track.rs"],
  ["crates/oxiguitar-layout/src/hands.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-layout/src/page.rs", "crates/oxiguitar-layout/src/plan.rs"],
  ["crates/oxiguitar-layout/src/effects.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-layout/src/effects.rs", "crates/oxiguitar-model/src/note.rs"],
  ["crates/oxiguitar-layout/src/effects.rs", "crates/oxiguitar-model/src/duration.rs"],
  // model internal
  ["crates/oxiguitar-model/src/score.rs", "crates/oxiguitar-model/src/track.rs"],
  ["crates/oxiguitar-model/src/score.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-model/src/track.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-model/src/track.rs", "crates/oxiguitar-model/src/tuning.rs"],
  ["crates/oxiguitar-model/src/track.rs", "crates/oxiguitar-model/src/duration.rs"],
  ["crates/oxiguitar-model/src/bar.rs", "crates/oxiguitar-model/src/note.rs"],
  ["crates/oxiguitar-model/src/bar.rs", "crates/oxiguitar-model/src/duration.rs"],
  ["crates/oxiguitar-model/src/bar.rs", "crates/oxiguitar-model/src/lyrics.rs"],
  ["crates/oxiguitar-model/src/note.rs", "crates/oxiguitar-model/src/duration.rs"],
  ["crates/oxiguitar-model/src/note.rs", "crates/oxiguitar-model/src/ratio.rs"],
  ["crates/oxiguitar-model/src/duration.rs", "crates/oxiguitar-model/src/ratio.rs"],
  ["crates/oxiguitar-model/src/lyrics.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-model/src/lyrics.rs", "crates/oxiguitar-model/src/note.rs"],
  ["crates/oxiguitar-model/src/wire.rs", "crates/oxiguitar-model/src/score.rs"],
  ["crates/oxiguitar-model/src/wire.rs", "crates/oxiguitar-model/src/track.rs"],
  ["crates/oxiguitar-model/src/wire.rs", "crates/oxiguitar-model/src/bar.rs"],
  ["crates/oxiguitar-model/src/wire.rs", "crates/oxiguitar-model/src/note.rs"],
  ["crates/oxiguitar-model/src/wire.rs", "crates/oxiguitar-model/src/tuning.rs"],
  ["crates/oxiguitar-model/src/wire.rs", "crates/oxiguitar-model/src/duration.rs"],
  ["crates/oxiguitar-model/src/wire.rs", "crates/oxiguitar-model/src/lyrics.rs"],
  ["crates/oxiguitar-model/src/wire.rs", "crates/oxiguitar-model/src/ratio.rs"],
];

for (const [fromFile, toFile] of crossFileImports) {
  const fromId = `file:${fromFile}`;
  const toId = `file:${toFile}`;
  if (nodeIds.has(fromId) && nodeIds.has(toId)) {
    edges.push({
      source: fromId, target: toId, type: "imports",
      direction: "forward", weight: 1,
    });
  }
}

// 7. Related/use edges between types
const relatedEdges = [
  // Model hierarchy: Score -> Track -> Bar -> Voice/Beat -> Note
  { s: "struct:crates/oxiguitar-model/src/score.rs:Score", t: "struct:crates/oxiguitar-model/src/track.rs:Track", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/track.rs:Track", t: "struct:crates/oxiguitar-model/src/bar.rs:Bar", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/bar.rs:Bar", t: "struct:crates/oxiguitar-model/src/bar.rs:Voice", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/bar.rs:Bar", t: "struct:crates/oxiguitar-model/src/bar.rs:Beat", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/bar.rs:Beat", t: "struct:crates/oxiguitar-model/src/note.rs:Note", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/bar.rs:Beat", t: "struct:crates/oxiguitar-model/src/bar.rs:BeatEffects", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/note.rs:Note", t: "struct:crates/oxiguitar-model/src/duration.rs:Duration", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/note.rs:Note", t: "struct:crates/oxiguitar-model/src/note.rs:NoteEffects", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/note.rs:Note", t: "struct:crates/oxiguitar-model/src/note.rs:Bend", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/note.rs:Note", t: "struct:crates/oxiguitar-model/src/note.rs:Harmonic", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/duration.rs:Duration", t: "struct:crates/oxiguitar-model/src/duration.rs:NoteValue", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/duration.rs:Duration", t: "struct:crates/oxiguitar-model/src/duration.rs:Tuplet", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/duration.rs:Duration", t: "struct:crates/oxiguitar-model/src/ratio.rs:Ratio", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/bar.rs:Bar", t: "struct:crates/oxiguitar-model/src/bar.rs:Clef", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/bar.rs:Bar", t: "struct:crates/oxiguitar-model/src/bar.rs:BeatKind", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/track.rs:Track", t: "struct:crates/oxiguitar-model/src/track.rs:Staff", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/track.rs:Track", t: "struct:crates/oxiguitar-model/src/track.rs:Instrument", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/track.rs:Track", t: "struct:crates/oxiguitar-model/src/track.rs:Mix", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/track.rs:Track", t: "struct:crates/oxiguitar-model/src/tuning.rs:Pattern", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/track.rs:Track", t: "struct:crates/oxiguitar-model/src/track.rs:ChordDiagram", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/score.rs:Score", t: "struct:crates/oxiguitar-model/src/score.rs:TimeSignature", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/score.rs:Score", t: "struct:crates/oxiguitar-model/src/score.rs:KeySignature", type: "uses" },
  { s: "struct:crates/oxiguitar-model/src/score.rs:Score", t: "struct:crates/oxiguitar-model/src/score.rs:MasterBar", type: "uses" },
  // Layout types
  { s: "struct:crates/oxiguitar-layout/src/lib.rs:Layout", t: "struct:crates/oxiguitar-layout/src/lib.rs:Paper", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/lib.rs:Layout", t: "struct:crates/oxiguitar-layout/src/lib.rs:Options", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/lib.rs:Layout", t: "struct:crates/oxiguitar-layout/src/lib.rs:Notation", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/lib.rs:Layout", t: "struct:crates/oxiguitar-layout/src/lib.rs:Engraved", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/engrave.rs:DrawnSystem", t: "struct:crates/oxiguitar-layout/src/engrave.rs:Stave", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/engrave.rs:Stave", t: "struct:crates/oxiguitar-layout/src/engrave.rs:Lane", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/engrave.rs:Stave", t: "struct:crates/oxiguitar-layout/src/engrave.rs:Band", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/plan.rs:BarPlan", t: "struct:crates/oxiguitar-layout/src/plan.rs:NoteInfo", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/plan.rs:BarPlan", t: "struct:crates/oxiguitar-layout/src/plan.rs:BeatInfo", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/plan.rs:BarPlan", t: "struct:crates/oxiguitar-layout/src/grid.rs:BarGrid", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/plan.rs:BarPlan", t: "struct:crates/oxiguitar-layout/src/engrave.rs:Lane", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/plan.rs:BarPlan", t: "struct:crates/oxiguitar-layout/src/engrave.rs:Band", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/system.rs:View", t: "struct:crates/oxiguitar-layout/src/system.rs:SystemSpec", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/hits.rs:Hits", t: "struct:crates/oxiguitar-layout/src/hits.rs:SystemHits", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/hits.rs:SystemHits", t: "struct:crates/oxiguitar-layout/src/hits.rs:BarHits", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/hits.rs:BarHits", t: "struct:crates/oxiguitar-layout/src/hits.rs:StaveHits", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/hits.rs:BarHits", t: "struct:crates/oxiguitar-layout/src/hits.rs:BeatHits", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/hits.rs:StaveHits", t: "struct:crates/oxiguitar-layout/src/hits.rs:TabHits", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/pitch.rs:Spelled", t: "struct:crates/oxiguitar-layout/src/pitch.rs:BarAccidentals", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/rhythm.rs:BeatRef", t: "struct:crates/oxiguitar-layout/src/rhythm.rs:Column", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/page.rs:Header", t: "struct:crates/oxiguitar-layout/src/lib.rs:Layout", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/draw.rs:G", t: "struct:crates/oxiguitar-layout/src/draw.rs:Painter", type: "uses" },
  // Cross-crate: layout uses model types
  { s: "struct:crates/oxiguitar-layout/src/plan.rs:BarPlan", t: "struct:crates/oxiguitar-model/src/bar.rs:Bar", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/plan.rs:BarPlan", t: "struct:crates/oxiguitar-model/src/note.rs:Note", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/plan.rs:BarPlan", t: "struct:crates/oxiguitar-model/src/duration.rs:Duration", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/system.rs:View", t: "struct:crates/oxiguitar-model/src/score.rs:Score", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/system.rs:View", t: "struct:crates/oxiguitar-model/src/track.rs:Track", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/hits.rs:Hits", t: "struct:crates/oxiguitar-model/src/bar.rs:Bar", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/hits.rs:Hits", t: "struct:crates/oxiguitar-model/src/note.rs:Note", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/pitch.rs:Spelled", t: "struct:crates/oxiguitar-model/src/bar.rs:Bar", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/pitch.rs:Spelled", t: "struct:crates/oxiguitar-model/src/note.rs:Note", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/rhythm.rs:BeatRef", t: "struct:crates/oxiguitar-model/src/bar.rs:Bar", type: "uses" },
  { s: "struct:crates/oxiguitar-layout/src/rhythm.rs:BeatRef", t: "struct:crates/oxiguitar-model/src/note.rs:Note", type: "uses" },
];

for (const re of relatedEdges) {
  if (nodeIds.has(re.s) && nodeIds.has(re.t)) {
    const exists = edges.some(e => e.source === re.s && e.target === re.t && e.type === re.type);
    if (!exists) {
      edges.push({ source: re.s, target: re.t, type: re.type, direction: "forward", weight: 1 });
    }
  }
}

// 8. Module -> type related edges
for (const [filePath, modules] of Object.entries(moduleMap)) {
  const modTypes = types[filePath] || [];
  for (const mod of modules) {
    const modId = `module:${filePath}:${mod}`;
    if (!nodeIds.has(modId)) continue;
    for (const t of modTypes) {
      const typeId = `${t.type}:${filePath}:${t.name}`;
      if (nodeIds.has(typeId)) {
        edges.push({
          source: modId, target: typeId, type: "related",
          direction: "forward", weight: 1,
        });
      }
    }
  }
}

// Deduplicate edges
const edgeSet = new Set();
const dedupedEdges = [];
for (const edge of edges) {
  const key = `${edge.source}|${edge.target}|${edge.type}`;
  if (!edgeSet.has(key)) {
    edgeSet.add(key);
    dedupedEdges.push(edge);
  }
}

const output = { nodes, edges: dedupedEdges };

writeFileSync(
  "/home/mat/Documents/src/oxiguitar/.ua/intermediate/batch-18.json",
  JSON.stringify(output, null, 2) + "\n",
  "utf-8"
);

console.log(`Batch 18: ${nodes.length} nodes, ${dedupedEdges.length} edges`);

const typeCounts = {};
for (const n of nodes) typeCounts[n.type] = (typeCounts[n.type] || 0) + 1;
console.log("Node types:", JSON.stringify(typeCounts, null, 2));

const edgeTypeCounts = {};
for (const e of dedupedEdges) edgeTypeCounts[e.type] = (edgeTypeCounts[e.type] || 0) + 1;
console.log("Edge types:", JSON.stringify(edgeTypeCounts, null, 2));
