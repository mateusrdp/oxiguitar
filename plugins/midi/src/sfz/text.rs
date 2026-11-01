// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reading an SFZ file's text into headers and opcodes, in order, as the
//! format's players read it (sfizz, the de facto reference, and ARIA):
//!
//! - `// …` to the end of the line and `/* … */` are comments.
//! - `<name>` is a header; `name=value` an opcode. A value runs to the end
//!   of its line, except that it stops before another header, a `#define`
//!   or `#include`, or another opcode (spaces, then a name and `=`): so
//!   `sample=Hit 1.wav lokey=36` is a file name with a space, then a key.
//! - `#define $NAME value` names a value (its first word); `$NAME` in a
//!   later opcode's name or value is replaced by it. A `$` takes the
//!   shortest run of letters, digits and underscores that is defined, so
//!   `$KEYsnare` is `$KEY` followed by `snare` when only `$KEY` is defined.
//! - `#include "file.sfz"` reads another file in place, its path relative
//!   to the folder of the top file (not of the file including it); a file
//!   may be included many times, up to [`MAX_DEPTH`] deep.

use std::path::{Path, PathBuf};

/// How deep includes may nest.
pub(crate) const MAX_DEPTH: usize = 32;

/// One header or opcode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    /// `<name>`.
    Header(String),
    /// `name=value`, with every `$NAME` replaced.
    Opcode(String, String),
}

/// A file's headers and opcodes, and what couldn't be read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Text {
    /// In the order they come, includes read in place.
    pub(crate) items: Vec<Item>,
    /// What was skipped, and why: a missing include, a broken line.
    pub(crate) problems: Vec<String>,
    /// Every file read: the top file and each one included.
    pub(crate) files: Vec<PathBuf>,
}

/// Reads the SFZ file `top` from the disk.
pub(crate) fn read(top: &Path) -> Result<Text, String> {
    read_with(top, &|p: &Path| std::fs::read(p))
}

/// Reads the SFZ file `top`, getting each file's bytes from `load` (the
/// disk, or a test's files).
pub(crate) fn read_with(
    top: &Path,
    load: &dyn Fn(&Path) -> std::io::Result<Vec<u8>>,
) -> Result<Text, String> {
    let bytes = load(top).map_err(|e| format!("cannot read {}: {e}", top.display()))?;
    let mut reader = Reader {
        base: top.parent().map(Path::to_path_buf).unwrap_or_default(),
        load,
        defines: Vec::new(),
        text: Text::default(),
    };
    reader.text.files.push(top.to_path_buf());
    reader.file(&bytes, 0);
    Ok(reader.text)
}

/// The paths a file includes, as written (`#include "…"`), without reading
/// them: to tell the files that are instruments from the ones they include.
pub(crate) fn includes(bytes: &[u8]) -> Vec<String> {
    let mut reader = Reader {
        base: PathBuf::new(),
        load: &|_: &Path| Err(std::io::Error::from(std::io::ErrorKind::Unsupported)),
        defines: Vec::new(),
        text: Text::default(),
    };
    let text = String::from_utf8_lossy(bytes);
    let mut found = Vec::new();
    // Each directive in order, so a path built from a `#define`d name
    // (`#include "$dir/env.sfz"`) is found too.
    for line in text.lines() {
        let mut rest = line;
        while let Some(at) = rest.find('#') {
            rest = &rest[at..];
            if let Some(after) = rest.strip_prefix("#include") {
                let after = after.trim_start_matches([' ', '\t']);
                if let Some(quoted) = after.strip_prefix('"')
                    && let Some(end) = quoted.find('"')
                {
                    found.push(reader.expand(&quoted[..end]).replace('\\', "/"));
                }
                rest = after;
            } else if rest.starts_with("#define") {
                let chars: Vec<char> = rest.chars().collect();
                let end = reader.directive(&chars, 0, 0);
                rest = &rest[chars[..end].iter().map(|c| c.len_utf8()).sum::<usize>()..];
            } else {
                rest = &rest[1..];
            }
        }
    }
    found
}

struct Reader<'a> {
    base: PathBuf,
    load: &'a dyn Fn(&Path) -> std::io::Result<Vec<u8>>,
    /// `#define`d names and their values, most recent last.
    defines: Vec<(String, String)>,
    text: Text,
}

fn identifier(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

fn space(c: char) -> bool {
    c == ' ' || c == '\t'
}

impl Reader<'_> {
    fn file(&mut self, bytes: &[u8], depth: usize) {
        let text = String::from_utf8_lossy(bytes);
        let chars: Vec<char> = text.chars().collect();
        let mut at = 0;
        while at < chars.len() {
            at = skip_blank(&chars, at);
            let Some(&c) = chars.get(at) else {
                break;
            };
            at = match c {
                '#' => self.directive(&chars, at, depth),
                '<' => self.header(&chars, at),
                _ => self.opcode(&chars, at),
            };
        }
    }

    fn problem(&mut self, what: String) {
        // A broken file could fill memory with complaints: the first few
        // say enough.
        if self.text.problems.len() < 100 {
            self.text.problems.push(what);
        }
    }

    /// `#define` or `#include` at `at`; returns where reading goes on.
    fn directive(&mut self, chars: &[char], at: usize, depth: usize) -> usize {
        let mut i = at + 1;
        let start = i;
        while i < chars.len() && identifier(chars[i]) {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        let line_end = end_of_line(chars, i);
        match word.as_str() {
            "define" => {
                let mut j = i;
                while j < line_end && space(chars[j]) {
                    j += 1;
                }
                if chars.get(j) != Some(&'$') {
                    self.problem("#define without a $name".into());
                    return line_end;
                }
                j += 1;
                let name_start = j;
                while j < line_end && identifier(chars[j]) {
                    j += 1;
                }
                let name: String = chars[name_start..j].iter().collect();
                while j < line_end && space(chars[j]) {
                    j += 1;
                }
                // ARIA's rule: the value is its first word; what follows
                // on the line is read on (another #define, often).
                let value_start = j;
                while j < line_end && !space(chars[j]) {
                    j += 1;
                }
                let value: String = chars[value_start..j].iter().collect();
                let value = strip_comment(&value).to_owned();
                if name.is_empty() {
                    self.problem("#define without a $name".into());
                } else {
                    self.defines.retain(|(n, _)| *n != name);
                    self.defines.push((name, value));
                }
                j
            }
            "include" => {
                let mut j = i;
                while j < line_end && space(chars[j]) {
                    j += 1;
                }
                if chars.get(j) != Some(&'"') {
                    self.problem("#include without a \"file\"".into());
                    return line_end;
                }
                let close = (j + 1..line_end).find(|&k| chars[k] == '"');
                let Some(close) = close else {
                    self.problem("#include without a closing quote".into());
                    return line_end;
                };
                let raw: String = chars[j + 1..close].iter().collect();
                let path = self.expand(&raw).replace('\\', "/");
                self.include(&path, depth);
                close + 1
            }
            _ => {
                self.problem(format!("unknown directive #{word}"));
                line_end
            }
        }
    }

    fn include(&mut self, path: &str, depth: usize) {
        if depth + 1 >= MAX_DEPTH {
            self.problem(format!("includes nest deeper than {MAX_DEPTH}: {path}"));
            return;
        }
        let full = self.base.join(path);
        match (self.load)(&full) {
            Ok(bytes) => {
                if !self.text.files.contains(&full) {
                    self.text.files.push(full);
                }
                self.file(&bytes, depth + 1);
            }
            Err(e) => self.problem(format!("cannot include {path}: {e}")),
        }
    }

    fn header(&mut self, chars: &[char], at: usize) -> usize {
        let line_end = end_of_line(chars, at);
        let close = (at + 1..line_end).find(|&k| chars[k] == '>');
        let Some(close) = close else {
            self.problem("a header without its closing >".into());
            return line_end;
        };
        let name: String = chars[at + 1..close].iter().collect();
        if name.is_empty() || !name.chars().all(identifier) {
            self.problem(format!("<{name}> isn't a header"));
        } else {
            self.text.items.push(Item::Header(name));
        }
        close + 1
    }

    fn opcode(&mut self, chars: &[char], at: usize) -> usize {
        let mut i = at;
        while i < chars.len() && (identifier(chars[i]) || chars[i] == '$') {
            i += 1;
        }
        if i == at || chars.get(i) != Some(&'=') {
            // Not an opcode: skip to the next space.
            let mut j = at.max(i);
            while j < chars.len() && !chars[j].is_whitespace() {
                j += 1;
            }
            let word: String = chars[at..j].iter().collect();
            self.problem(format!("'{word}' isn't an opcode"));
            return j.max(at + 1);
        }
        let raw_name: String = chars[at..i].iter().collect();
        let value_start = i + 1;
        let value_end = value_end(chars, value_start);
        let raw_value: String = chars[value_start..value_end].iter().collect();
        let name = self.expand(&raw_name);
        let value = self.expand(raw_value.trim_end());
        self.text.items.push(Item::Opcode(name, value));
        value_end
    }

    /// `text` with every defined `$NAME` replaced (ARIA's rule: the
    /// shortest name that is defined).
    fn expand(&mut self, text: &str) -> String {
        if !text.contains('$') {
            return text.to_owned();
        }
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < chars.len() {
            if chars[i] != '$' {
                out.push(chars[i]);
                i += 1;
                continue;
            }
            i += 1;
            let mut name = String::new();
            let mut value = None;
            while i < chars.len() && identifier(chars[i]) {
                name.push(chars[i]);
                i += 1;
                if let Some((_, v)) = self.defines.iter().rev().find(|(n, _)| *n == name) {
                    value = Some(v.clone());
                    break;
                }
            }
            match value {
                Some(v) => out.push_str(&v),
                None => {
                    if self.text.problems.len() < 100 {
                        self.text.problems.push(format!("${name} isn't defined"));
                    }
                    out.push('$');
                    out.push_str(&name);
                }
            }
        }
        out
    }
}

/// Where the blank (spaces, line ends, comments) from `at` ends.
fn skip_blank(chars: &[char], mut at: usize) -> usize {
    loop {
        while at < chars.len() && chars[at].is_whitespace() {
            at += 1;
        }
        if chars.get(at) == Some(&'/') && chars.get(at + 1) == Some(&'/') {
            at = end_of_line(chars, at);
        } else if chars.get(at) == Some(&'/') && chars.get(at + 1) == Some(&'*') {
            at = end_of_block_comment(chars, at + 2);
        } else {
            return at;
        }
    }
}

fn end_of_line(chars: &[char], mut at: usize) -> usize {
    while at < chars.len() && chars[at] != '\n' && chars[at] != '\r' {
        at += 1;
    }
    at
}

fn end_of_block_comment(chars: &[char], mut at: usize) -> usize {
    while at < chars.len() {
        if chars[at] == '*' && chars.get(at + 1) == Some(&'/') {
            return at + 2;
        }
        at += 1;
    }
    at
}

/// `text` up to a comment.
fn strip_comment(text: &str) -> &str {
    let cut = [text.find("//"), text.find("/*")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(text.len());
    &text[..cut]
}

/// Where an opcode's value that starts at `start` ends: the end of the
/// line or a comment, or before a header, a directive or another opcode
/// that follows spaces.
fn value_end(chars: &[char], start: usize) -> usize {
    let mut line_end = start;
    while line_end < chars.len() {
        let c = chars[line_end];
        if c == '\n' || c == '\r' {
            break;
        }
        if c == '/' && matches!(chars.get(line_end + 1), Some('/' | '*')) {
            break;
        }
        line_end += 1;
    }
    let mut end = start;
    while end < line_end {
        let c = chars[end];
        if c == '<' {
            break;
        }
        if space(c) {
            let mut i = end + 1;
            while i < line_end && space(chars[i]) {
                i += 1;
            }
            if i == line_end || chars[i] == '<' {
                break;
            }
            if chars[i] == '#' {
                let word: String = chars[i + 1..line_end]
                    .iter()
                    .take_while(|&&c| identifier(c))
                    .collect();
                if word == "define" || word == "include" {
                    break;
                }
            }
            if identifier(chars[i]) {
                let mut j = i + 1;
                while j < line_end && (identifier(chars[j]) || chars[j] == '$') {
                    j += 1;
                }
                if chars.get(j) == Some(&'=') && j < line_end {
                    break;
                }
            }
            end = i;
            continue;
        }
        end += 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[(&str, &str)]) -> impl Fn(&Path) -> std::io::Result<Vec<u8>> + use<> {
        let list: Vec<(PathBuf, Vec<u8>)> = list
            .iter()
            .map(|(p, t)| (PathBuf::from(p), t.as_bytes().to_vec()))
            .collect();
        move |p: &Path| {
            list.iter()
                .find(|(q, _)| q == p)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        }
    }

    fn op(n: &str, v: &str) -> Item {
        Item::Opcode(n.into(), v.into())
    }

    fn header(n: &str) -> Item {
        Item::Header(n.into())
    }

    #[test]
    fn values_run_to_the_next_opcode_header_or_comment() {
        let load = files(&[(
            "/k/i.sfz",
            "<region> sample=Hit 1.wav lokey=36 hikey=c#4 // a comment\n\
             <region>sample=x.wav<region> pan=-10 /* a\nblock */ volume=-3  \n\
             label_cc7=Release vol tune=5",
        )]);
        let t = read_with(Path::new("/k/i.sfz"), &load).unwrap();
        assert_eq!(
            t.items,
            [
                header("region"),
                op("sample", "Hit 1.wav"),
                op("lokey", "36"),
                op("hikey", "c#4"),
                header("region"),
                op("sample", "x.wav"),
                header("region"),
                op("pan", "-10"),
                op("volume", "-3"),
                op("label_cc7", "Release vol"),
                op("tune", "5"),
            ]
        );
        assert!(t.problems.is_empty(), "{:?}", t.problems);
    }

    #[test]
    fn defines_replace_names_and_values_by_the_shortest_defined_name() {
        let load = files(&[(
            "/k/i.sfz",
            "#define $KEY 36 // the kick\n#define $CC 7    #define $V1H 27\n\
             <region> key=$KEY amplitude_oncc$CC=50 sample=$KEYsnare.wav lokey=$NONE hivel=$V1H",
        )]);
        let t = read_with(Path::new("/k/i.sfz"), &load).unwrap();
        assert_eq!(
            t.items,
            [
                header("region"),
                op("key", "36"),
                op("amplitude_oncc7", "50"),
                op("sample", "36snare.wav"),
                op("lokey", "$NONE"),
                op("hivel", "27"),
            ]
        );
        assert_eq!(t.problems, ["$NONE isn't defined"]);
    }

    #[test]
    fn includes_come_from_the_top_files_folder_and_may_repeat() {
        let load = files(&[
            (
                "/k/Programs/kit.sfz",
                "<global> #include \"modules/env.sfz\"\n<group> #include \"modules/env.sfz\" #include \"gone.sfz\"",
            ),
            (
                "/k/Programs/modules/env.sfz",
                "ampeg_release=0.3 #include \"modules/deeper.sfz\"",
            ),
            ("/k/Programs/modules/deeper.sfz", "volume=1"),
        ]);
        let t = read_with(Path::new("/k/Programs/kit.sfz"), &load).unwrap();
        assert_eq!(
            t.items,
            [
                header("global"),
                op("ampeg_release", "0.3"),
                op("volume", "1"),
                header("group"),
                op("ampeg_release", "0.3"),
                op("volume", "1"),
            ]
        );
        assert_eq!(t.files.len(), 3);
        assert_eq!(t.problems.len(), 1);
        assert!(t.problems[0].starts_with("cannot include gone.sfz"));
    }

    #[test]
    fn an_include_of_itself_stops_at_the_depth_limit() {
        let load = files(&[("/k/i.sfz", "<region> #include \"i.sfz\"")]);
        let t = read_with(Path::new("/k/i.sfz"), &load).unwrap();
        assert_eq!(t.items.len(), MAX_DEPTH);
        assert_eq!(t.problems.len(), 1);
    }

    #[test]
    fn junk_is_skipped_and_said() {
        let load = files(&[("/k/i.sfz", "<region> what <bad name> lokey=1\n#pragma once")]);
        let t = read_with(Path::new("/k/i.sfz"), &load).unwrap();
        assert_eq!(t.items, [header("region"), op("lokey", "1")]);
        assert_eq!(t.problems.len(), 3, "{:?}", t.problems);
    }

    #[test]
    fn includes_are_listed_without_reading_them() {
        let text = b"<global>\n#include \"modules\\env.sfz\"\n  #include  \"maps/ord.sfz\" // x\n\
                     #define $dir kit #define $x 1\n#include \"$dir/a.sfz\"";
        assert_eq!(
            includes(text),
            ["modules/env.sfz", "maps/ord.sfz", "kit/a.sfz"]
        );
    }
}
