# Exporters

Since interface 1.4, a plugin can add entries to **File → Export**. An exporter of kind `audio` writes the score's sound to a file: OxiGuitar plays the score through the user's instruments, amps and mixer, faster than real time, and hands your plugin the sound in pieces; your plugin turns it into the file's bytes, and OxiGuitar writes them. Your plugin never opens a file itself.

## In the manifest

List each exporter under `exporters`, with an `id`, the `name` of what it makes (the menu shows "Export WAV Audio…"), the files' `extension` (lowercase, without the dot) and its `kind`, `"audio"`:

```json
"exporters": [
  { "id": "flac", "name": "FLAC Audio", "extension": "flac", "kind": "audio" }
]
```

A manifest with `exporters` must ask for interface 1.4.

## In the plugin

Your `Plugin` hands out each one from `audio_exporter(id)`, as something implementing `AudioExporter`. An export goes:

1. **`start`**: return the rate you want the sound at (8000 to 384000 frames a second; your settings window can let the user pick) and the file's first bytes, a header, perhaps empty.
2. **`samples`**, again and again: stereo frames, left and right interleaved, at that rate, as the mixer leaves them (full scale is ±1; louder is possible, so clip or scale as your format needs). Return the bytes that follow in the file.
3. **`finish`**: return the file's last bytes and, if your header names the file's length (WAV's does), the header again in `start`: OxiGuitar writes it over the beginning once everything else is written. With `cancelled`, the user stopped the export and the file is thrown away; just forget it.

One export runs at a time, so keep the one under way in your exporter, behind a `Mutex`, and drop it on a new `start`. The calls come from one thread of OxiGuitar's, never the audio thread: you may allocate and take your time, though the user is watching a progress bar.

OxiGuitar writes the file under a temporary name and renames it at the end, so a failed or cancelled export never leaves half a file, and an existing file is replaced only by a whole one.

## What the user chooses

Before the file dialog, OxiGuitar asks what to export: the whole song or the selected bars, at the written tempo or the practice speed, with or without the metronome. Each track plays with its sound, amp and mixer settings as they are. Two seconds follow the end, so the last notes ring out. The file's own options (bit depth, rate, compression) are yours: put them in your [settings window](settings.md).

`oxiguitar-cli export FILE NEW-FILE` exports the same way, through the exporter whose extension the new file has.

## Testing

The contract suite ([Testing](testing.md)) runs each audio exporter through a second of sound in pieces of uneven sizes, an export cancelled halfway, and a finish with no export started, and checks that what you ask to write over the start is no longer than the file.
