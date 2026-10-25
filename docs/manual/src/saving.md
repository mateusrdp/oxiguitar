# Saving files

Choose **File → Save** (Ctrl + S) to write the score back to its file, or **File → Save As…** (Ctrl + Shift + S) to write it under another name. The window's title shows the file's name, with a dot before it while the score has changes not yet saved.

OxiGuitar saves through its format plugins, as it opens files. Ours write Guitar Pro 7 and 8 files (.gp), which Guitar Pro 7 and later open, and Guitar Pro 5 files (.gp5), which Guitar Pro 5 and every later version open, and TuxGuitar too. Choose the format in Save As… by the name's extension. Save keeps a file's own format unless that format can't hold everything in the score, or no installed plugin writes it (Guitar Pro 3 and 4 files and .gpx are only read): then the score is saved beside the file, under the same name, as a .gp file, the fullest format OxiGuitar writes, and the original file stays as it was. OxiGuitar asks before doing so, and offers Save As… to choose another name.

## What a file can't hold

No format holds everything. Before writing a file that would lose something, OxiGuitar lists what, with the first bar and track where it happens, and asks whether to save. For a .gp file the list is usually short, such as:

- different dynamics on the notes of one beat (Guitar Pro 7 has one dynamic per beat);
- the colours of sections;
- trills at a speed other than sixteenths;
- bends with more points than Guitar Pro keeps (a bend, a hold and a release fit).

A .gp5 file loses more, because the format is older:

- the second and further staves of a track, and the third and fourth voices of a bar;
- tuplets other than the common ones (3 in the time of 2, 5 to 7 in the time of 4, 9 to 13 in the time of 8), double dots, double-whole and 128th notes;
- bars in free time, signs (segno, coda, D.S. and the like) in more than one bar, alternate endings above the eighth, repeat counts above 99, section letters and tempo names;
- tempo changes that aren't at the start of a beat, and gradual tempos;
- left-hand tapping, feedback harmonics, artificial harmonics other than one, two or three octaves up, and bends finer than its steps;
- lyrics of more than one track or of more than five verses, and characters outside Windows-1252 (written as ?).

A few things are kept in another shape and not listed as losses, since they come back as they were: dynamic markings are written as the notes' dynamics, and volume and pan are kept to sixteen steps.

When you save over the file the score came from, the list also names what OxiGuitar couldn't keep when it opened the file, since those are lost from the file once it is written over.

To try a file without changing it, use Save As.

## Plugins' settings for each track

Some plugins add their own settings to a track's **Properties…** (the NAM plugin's amp model, for example). No score format holds them, so OxiGuitar keeps them in a small file beside the score, named after it: `song.gp3` keeps them in `song.gp3.oxg`. Saving writes it and opening reads it; keep the two files together when you copy or share a song. Without the .oxg file the score opens as usual, with those settings empty. Guitar Pro and other programs ignore it.

## Before closing

Opening another file, quitting, or closing the window with changes not yet saved asks first: **Save**, **Don't Save** (the changes are lost), or **Cancel**.

## From the command line

`oxiguitar-cli convert song.gp song.gp5` writes a file in the format of the new file's extension and prints what it couldn't hold. It copies `song.gp.oxg`, if there is one, to `song.gp5.oxg`.
