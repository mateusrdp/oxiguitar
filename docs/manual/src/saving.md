# Saving files

Choose **File → Save** (Ctrl + S) to write the score back to its file, or **File → Save As…** (Ctrl + Shift + S) to write it under another name. The window's title shows the file's name, with a dot before it while the score has changes not yet saved.

OxiGuitar saves through its format plugins, as it opens files. Ours write Guitar Pro 7 and 8 files (.gp), which Guitar Pro 7 and later open; Guitar Pro 5 files (.gp5) come later. Save keeps a file's own format unless that format can't hold everything in the score, or no installed plugin writes it: then the score is saved beside the file, under the same name, as a .gp file, the fullest format OxiGuitar writes, and the original file stays as it was. OxiGuitar asks before doing so.

## What a file can't hold

No format holds everything. Before writing a file that would lose something, OxiGuitar lists what, with the first bar and track where it happens, and asks whether to save. For a .gp file the list is usually short, such as:

- different dynamics on the notes of one beat (Guitar Pro 7 has one dynamic per beat);
- the colours of sections;
- trills at a speed other than sixteenths;
- bends with more points than Guitar Pro keeps (a bend, a hold and a release fit).

When you save over the file the score came from, the list also names what OxiGuitar couldn't keep when it opened the file, since those are lost from the file once it is written over.

To try a file without changing it, use Save As.

## Before closing

Opening another file, quitting, or closing the window with changes not yet saved asks first: **Save**, **Don't Save** (the changes are lost), or **Cancel**.

## From the command line

`oxiguitar-cli convert song.gp5 song.gp` writes a file in the format of the new file's extension and prints what it couldn't hold.
