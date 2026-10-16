# Opening files

Choose **File → Open…** (Ctrl + O), drag a file from your file manager onto the window, double-click a Guitar Pro file if your package registered them ([Installing](installing.md)), or name the file when you start the program:

```sh
oxiguitar song.gp
```

OxiGuitar opens Guitar Pro files of every version from 3 to 8:

| Extension | Guitar Pro version |
| --- | --- |
| .gp | 7 and 8 |
| .gpx | 6 |
| .gp5 | 5 |
| .gp4, .gp3 | 4 and 3 |

It reads a file by its contents, so a file with the wrong extension still opens. The program shows what the file says: a tab saved without key signatures is shown in C, and each track shows the notation it was saved with (you can change this in the [track list](tracks.md)).

MuseScore (.mscz, .mscx), MusicXML, ABC and MIDI files will be opened by format plugins in a later version. Until then the status line at the bottom left says so. It also explains what went wrong when a file cannot be read; click it to close it.

Dragging a file onto the window works on Windows, macOS and Linux desktops running X11; on Wayland desktops (the default on GNOME and KDE) it does not work yet; open the file with File → Open or a double-click instead. If you drop several files, the first one opens.

On Linux the Open dialog comes from your desktop through the XDG desktop portal, which GNOME and KDE install by default. Without it, the status line says so, and you can still open files from the command line.
