// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Writes the OxiGuitar test song (the same music as `gp_writer.rs`) as GPIF
//! XML, in Guitar Pro 6 and Guitar Pro 7 dialects, and packs it as a .gpx
//! (BCFS file system, BCFZ-compressed) or a .gp (ZIP archive).
//!
//! Besides the shared music, the GPIF song has a few things Guitar Pro 5
//! cannot store: a rehearsal letter, a chord diagram, a brush stroke and a
//! tempo change in the middle of a bar.

use std::fmt::Write as _;
use std::io::Write as _;

/// Which Guitar Pro writes the GPIF.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    Gp6,
    Gp7,
}

/// A note: model string (1 = highest), fret, and its extra XML.
struct N(u8, u8, &'static str);

/// The test song's score.gpif.
pub(crate) fn score_gpif(dialect: Dialect) -> String {
    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<GPIF>\n");
    if dialect == Dialect::Gp7 {
        x.push_str("<GPVersion>7</GPVersion>\n<GPRevision required=\"12021\" recommended=\"12023\">12025</GPRevision>\n");
        x.push_str("<Encoding><EncodingDescription>GP7</EncodingDescription></Encoding>\n");
    } else {
        x.push_str("<GPRevision>7775</GPRevision>\n");
    }
    x.push_str("<Score>\n");
    for (tag, text) in [
        ("Title", super::gp_writer::TITLE),
        ("SubTitle", ""),
        ("Artist", super::gp_writer::ARTIST),
        ("Album", "Fixtures"),
        ("Words", "OxiGuitar"),
        ("Music", ""),
        ("WordsAndMusic", "OxiGuitar"),
        ("Copyright", "Public domain"),
        ("Tabber", "OxiGuitar tests"),
        ("Instructions", ""),
        ("Notices", "Made for OxiGuitar's tests"),
    ] {
        let _ = writeln!(x, "<{tag}><![CDATA[{text}]]></{tag}>");
    }
    x.push_str("</Score>\n<MasterTrack>\n<Tracks>0</Tracks>\n<Automations>\n");
    let _ = writeln!(
        x,
        "<Automation><Type>Tempo</Type><Linear>false</Linear><Bar>0</Bar><Position>0</Position>\
         <Visible>true</Visible><Value>{} 2</Value><Text>Moderato</Text></Automation>",
        super::gp_writer::TEMPO
    );
    // Half a bar into bar 3 (3/4), in eighth notes: 160 eighths a minute = 80 quarters.
    x.push_str(
        "<Automation><Type>Tempo</Type><Linear>false</Linear><Bar>2</Bar><Position>0.5</Position>\
         <Visible>true</Visible><Value>160 1</Value></Automation>\n",
    );
    x.push_str("</Automations>\n</MasterTrack>\n<Tracks>\n<Track id=\"0\">\n");
    x.push_str("<Name><![CDATA[Guitar]]></Name>\n<ShortName><![CDATA[gtr.]]></ShortName>\n<Color>255 0 0</Color>\n");
    let tuning = "<Property name=\"Tuning\"><Pitches>40 45 50 55 59 64</Pitches></Property>";
    let diagrams = "<Property name=\"DiagramCollection\"><Items>\
        <Item id=\"0\" name=\"E5\"><Diagram stringCount=\"6\" fretCount=\"5\" baseFret=\"0\">\
        <Fret string=\"0\" fret=\"0\"/><Fret string=\"1\" fret=\"2\"/><Fret string=\"2\" fret=\"2\"/>\
        </Diagram></Item></Items></Property>";
    match dialect {
        Dialect::Gp7 => {
            x.push_str("<InstrumentSet><Name>Electric Guitar</Name><Type>electricGuitar</Type><LineCount>5</LineCount></InstrumentSet>\n");
            let _ = writeln!(
                x,
                "<Staves><Staff><Properties><Property name=\"CapoFret\"><Fret>0</Fret></Property>\
                 <Property name=\"FretCount\"><Number>24</Number></Property>{tuning}{diagrams}\
                 </Properties></Staff></Staves>"
            );
            x.push_str("<Transpose><Chromatic>0</Chromatic><Octave>-1</Octave></Transpose>\n");
            x.push_str("<Sounds><Sound><Name>Clean</Name><MIDI><LSB>0</LSB><MSB>0</MSB><Program>27</Program></MIDI></Sound></Sounds>\n");
            x.push_str("<MidiConnection><Port>0</Port><PrimaryChannel>0</PrimaryChannel><SecondaryChannel>1</SecondaryChannel></MidiConnection>\n");
        }
        Dialect::Gp6 => {
            x.push_str("<Instrument ref=\"e-gtr6\"/>\n");
            x.push_str("<GeneralMidi table=\"Instrument\"><Program>27</Program><Port>0</Port><PrimaryChannel>0</PrimaryChannel><SecondaryChannel>1</SecondaryChannel></GeneralMidi>\n");
            x.push_str("<PartSounding><NominalKey>C</NominalKey><TranspositionPitch>-12</TranspositionPitch></PartSounding>\n");
            let _ = writeln!(
                x,
                "<Properties>{tuning}<Property name=\"CapoFret\"><Fret>0</Fret></Property>{diagrams}</Properties>"
            );
        }
    }
    x.push_str("<RSE><ChannelStrip version=\"E56\"><Parameters>0.5 0.5 0.5 0.5 0.5 0.5 0.5 0.5 0.5 0 0.5 0.5 0.8 0.5 0.5 0.5</Parameters></ChannelStrip></RSE>\n");
    x.push_str("<Lyrics dispatched=\"true\"><Line><Text><![CDATA[la la la]]></Text><Offset>0</Offset></Line><Line><Text><![CDATA[]]></Text><Offset>0</Offset></Line></Lyrics>\n");
    x.push_str("<PlaybackState>Default</PlaybackState>\n</Track>\n</Tracks>\n");

    x.push_str("<MasterBars>\n");
    x.push_str(
        "<MasterBar><Key><AccidentalCount>0</AccidentalCount><Mode>Major</Mode></Key><Time>4/4</Time><Bars>0</Bars>\
         <Repeat start=\"true\" end=\"false\" count=\"0\"/>\
         <Section><Letter><![CDATA[A]]></Letter><Text><![CDATA[Intro]]></Text></Section></MasterBar>\n",
    );
    x.push_str(
        "<MasterBar><Key><AccidentalCount>0</AccidentalCount><Mode>Major</Mode></Key><Time>4/4</Time><Bars>1</Bars>\
         <Repeat start=\"false\" end=\"true\" count=\"2\"/></MasterBar>\n",
    );
    x.push_str(
        "<MasterBar><Key><AccidentalCount>0</AccidentalCount><Mode>Major</Mode></Key><Time>3/4</Time><Bars>2</Bars></MasterBar>\n",
    );
    x.push_str("</MasterBars>\n<Bars>\n");
    for (id, voices) in ["0 -1 -1 -1", "1 -1 -1 -1", "2 3 -1 -1"].iter().enumerate() {
        let _ = writeln!(
            x,
            "<Bar id=\"{id}\"><Clef>G2</Clef><Voices>{voices}</Voices></Bar>"
        );
    }
    x.push_str("</Bars>\n<Voices>\n");
    for (id, beats) in ["0 1 2 3 4 5", "6 7", "8", "9 10 11"].iter().enumerate() {
        let _ = writeln!(x, "<Voice id=\"{id}\"><Beats>{beats}</Beats></Voice>");
    }
    x.push_str("</Voices>\n");

    // Beats: rhythm id, notes, extra XML.
    let bend = "<Property name=\"Bended\"><Enable/></Property>\
        <Property name=\"BendOriginValue\"><Float>0.000000</Float></Property>\
        <Property name=\"BendOriginOffset\"><Float>0.000000</Float></Property>\
        <Property name=\"BendMiddleValue\"><Float>100.000000</Float></Property>\
        <Property name=\"BendMiddleOffset1\"><Float>50.000000</Float></Property>\
        <Property name=\"BendMiddleOffset2\"><Float>50.000000</Float></Property>\
        <Property name=\"BendDestinationValue\"><Float>0.000000</Float></Property>\
        <Property name=\"BendDestinationOffset\"><Float>100.000000</Float></Property>";
    let pm = "<Property name=\"PalmMuted\"><Enable/></Property>";
    let beats: Vec<(u8, Vec<N>, &str)> = vec![
        (
            0,
            vec![N(
                1,
                0,
                "<Property name=\"HopoOrigin\"><Enable/></Property>",
            )],
            "",
        ),
        (1, vec![N(2, 1, "")], ""),
        (1, vec![N(2, 3, "")], ""),
        (1, vec![N(2, 5, bend)], ""),
        (0, vec![N(3, 2, "")], "<FreeText>let it ring</FreeText>"),
        (0, vec![N(3, 2, "")], ""),
        (
            2,
            vec![N(6, 0, "<Property name=\"Muted\"><Enable/></Property>")],
            "",
        ),
        (2, vec![], ""),
        (
            3,
            vec![N(4, 2, pm), N(5, 2, pm), N(6, 0, pm)],
            "<Chord>0</Chord><Properties><Property name=\"Brush\"><Direction>Down</Direction></Property></Properties>",
        ),
        (0, vec![N(1, 0, "")], ""),
        (0, vec![N(1, 1, "")], ""),
        (0, vec![N(1, 3, "")], ""),
    ];
    x.push_str("<Beats>\n");
    let mut note_id = 0;
    let mut notes_xml = String::new();
    for (id, (rhythm, notes, extra)) in beats.iter().enumerate() {
        let ids: Vec<String> = (0..notes.len())
            .map(|k| (note_id + k).to_string())
            .collect();
        let _ = write!(
            x,
            "<Beat id=\"{id}\"><Dynamic>MF</Dynamic><Rhythm ref=\"{rhythm}\"/>"
        );
        if !notes.is_empty() {
            let _ = write!(x, "<Notes>{}</Notes>", ids.join(" "));
        }
        let _ = writeln!(x, "{extra}</Beat>");
        for n in notes {
            // GPIF numbers strings from the lowest: model string 1 is 5.
            let gpif_string = 6 - n.0;
            let tie = match (id, n.0) {
                (4, 3) => "<Tie origin=\"true\" destination=\"false\"/>",
                (5, 3) => "<Tie origin=\"false\" destination=\"true\"/>",
                _ => "",
            };
            let _ = writeln!(
                notes_xml,
                "<Note id=\"{note_id}\">{tie}<Properties><Property name=\"String\"><String>{gpif_string}</String></Property>\
                 <Property name=\"Fret\"><Fret>{}</Fret></Property>{}</Properties></Note>",
                n.1, n.2
            );
            note_id += 1;
        }
    }
    x.push_str("</Beats>\n<Notes>\n");
    x.push_str(&notes_xml);
    x.push_str("</Notes>\n<Rhythms>\n");
    x.push_str("<Rhythm id=\"0\"><NoteValue>Quarter</NoteValue></Rhythm>\n");
    x.push_str("<Rhythm id=\"1\"><NoteValue>Eighth</NoteValue><PrimaryTuplet num=\"3\" den=\"2\"/></Rhythm>\n");
    x.push_str("<Rhythm id=\"2\"><NoteValue>Half</NoteValue></Rhythm>\n");
    x.push_str(
        "<Rhythm id=\"3\"><NoteValue>Half</NoteValue><AugmentationDot count=\"1\"/></Rhythm>\n",
    );
    x.push_str("</Rhythms>\n</GPIF>\n");
    x
}

/// PartConfiguration for one track showing standard notation and tablature.
pub(crate) fn part_configuration() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&2u32.to_be_bytes());
    for _ in 0..2 {
        v.push(0);
        v.extend_from_slice(&1u32.to_be_bytes());
        v.push(3);
    }
    v.extend_from_slice(&1u32.to_be_bytes());
    v
}

/// The test song as a Guitar Pro 7 file (.gp).
pub(crate) fn test_song_gp() -> Vec<u8> {
    pack_zip(&score_gpif(Dialect::Gp7))
}

/// A .gp archive around any score.gpif text.
pub(crate) fn pack_zip(gpif: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    // A fixed date keeps the fixture byte-for-byte reproducible.
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    zip.start_file("VERSION", opts).unwrap();
    zip.write_all(b"7.0").unwrap();
    zip.start_file("Content/score.gpif", opts).unwrap();
    zip.write_all(gpif.as_bytes()).unwrap();
    zip.start_file("Content/PartConfiguration", opts).unwrap();
    zip.write_all(&part_configuration()).unwrap();
    zip.finish().unwrap().into_inner()
}

/// The test song as a Guitar Pro 6 file (.gpx).
pub(crate) fn test_song_gpx() -> Vec<u8> {
    bcfz(&bcfs(&[
        ("score.gpif", score_gpif(Dialect::Gp6).into_bytes()),
        ("misc.xml", b"<?xml version=\"1.0\"?><Misc/>".to_vec()),
        ("PartConfiguration", part_configuration()),
    ]))
}

const SECTOR: usize = 0x1000;

/// A BCFS file system: "BCFS", then 4 KiB sectors. Sector 0 is left empty,
/// each file gets an entry sector followed by its data sectors.
pub(crate) fn bcfs(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut sectors: Vec<Vec<u8>> = vec![vec![0; SECTOR]];
    for (name, data) in files {
        let entry_index = sectors.len();
        sectors.push(vec![0; SECTOR]);
        let mut list = Vec::new();
        for chunk in data.chunks(SECTOR) {
            list.push(sectors.len() as u32);
            let mut s = chunk.to_vec();
            s.resize(SECTOR, 0);
            sectors.push(s);
        }
        let e = &mut sectors[entry_index];
        e[0..4].copy_from_slice(&2u32.to_le_bytes());
        e[4..4 + name.len()].copy_from_slice(name.as_bytes());
        e[0x8C..0x90].copy_from_slice(&(data.len() as u32).to_le_bytes());
        for (k, s) in list.iter().enumerate() {
            e[0x94 + 4 * k..0x98 + 4 * k].copy_from_slice(&s.to_le_bytes());
        }
    }
    let mut out = b"BCFS".to_vec();
    for s in sectors {
        out.extend_from_slice(&s);
    }
    out
}

/// BCFZ compression with a simple greedy search for repeats, so the reader's
/// back-reference path is exercised.
pub(crate) fn bcfz(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::default();
    let mut i = 0;
    while i < data.len() {
        let (mut best_len, mut best_off) = (0, 0);
        for off in 1..=i.min(4095) {
            let start = i - off;
            let mut len = 0;
            // Copies never overlap their own output (size <= offset).
            while len < off
                && i + len < data.len()
                && data[start + len] == data[i + len]
                && len < 4095
            {
                len += 1;
            }
            if len > best_len {
                (best_len, best_off) = (len, off);
            }
        }
        if best_len >= 4 {
            let word = 32 - (best_off.max(best_len) as u32).leading_zeros();
            w.bit(1);
            w.bits(word, 4);
            w.bits_reversed(best_off as u32, word);
            w.bits_reversed(best_len as u32, word);
            i += best_len;
        } else {
            let n = (data.len() - i).min(3);
            w.bit(0);
            w.bits_reversed(n as u32, 2);
            for &b in &data[i..i + n] {
                w.bits(u32::from(b), 8);
            }
            i += n;
        }
    }
    let mut out = b"BCFZ".to_vec();
    out.extend_from_slice(&(data.len() as i32).to_le_bytes());
    out.extend_from_slice(&w.out);
    out
}

#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    n: usize,
}

impl BitWriter {
    fn bit(&mut self, b: u32) {
        if self.n.is_multiple_of(8) {
            self.out.push(0);
        }
        if b != 0 {
            *self.out.last_mut().unwrap() |= 1 << (7 - self.n % 8);
        }
        self.n += 1;
    }
    fn bits(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            self.bit((v >> i) & 1);
        }
    }
    fn bits_reversed(&mut self, v: u32, n: u32) {
        for i in 0..n {
            self.bit((v >> i) & 1);
        }
    }
}
