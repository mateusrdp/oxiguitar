# 0014 — Plugin files are tar archives

- Status: accepted (Mateus, 24 Nov 2026)
- Date: 2026-11-24
- Milestone: v1.0 (#347, from #346)
- Replaces: the "one native library file" of ADR 0008, as to what the file is
- Goes on from: ADR 0013 (plugins' UI in interpreted Slint)

## Context

A plugin file was its library, renamed. Interface 1.15 (ADR 0013) had a plugin hand over its `.slint` file as a string compiled into the library, which can't import other `.slint` files or use pictures. The AI plugin will soon need assets of its own. Mateus, 24 Nov 2026: compiled-in strings are not elegant; future-proof the plugin interface; a plugin file should be an archive, renamed, holding any files; startup speed matters far more than size.

## Decision

- A `.oxp` file is an **uncompressed tar archive** (ustar or GNU): `manifest.json` first, the library second (`plugin.so`, `plugin.dll`, `plugin.dylib`), then any files. Tar because each member is stored as it is after a 512-byte header, so reading the manifest is reading two headers and a few kilobytes, and unpacking is copying; zip would mean a central directory at the end and, compressed, decompression at first load. The release zips and packages compress the files anyway.
- The library keeps its manifest section, which must match `manifest.json`, and the existing checks (system, processor, truncation) run on the library member in place.
- At start, nothing is unpacked. The copy that is used is unpacked the first time it is loaded into the user's data folder (`unpacked/ID/PLACE-VERSION-SIZE-TIME/`) and **kept**; a later start finds it with one `stat` and unpacks again only when the chosen file changes. Unpacking goes through a temporary folder renamed into place; links, absolute paths and `..` are refused.
- **One copy per plugin id, by folder** (Mateus: "bindir > homedir > system, regardless of version", for downgrades and picking a version): the later folder of the search list wins; in one folder, the higher version, then processor features, then the name.
- **Archives only**: the old bare-library files are refused, saying they are of the old format.
- Interface 1.16 lets a plugin use what it packs: the manifest's `ui` names its `.slint` file, compiled from the unpacked folder, and `HostRequest::OwnFiles` gives it that folder. A 1.15 plugin's `Request::Ui` stays, since 0.22.0 released it.

## Consequences

- A plugin ships any files, and its UI is an ordinary Slint project: imports and pictures work.
- The first load of a plugin after it is installed or changed copies its file once (a few milliseconds for a few megabytes); later starts cost what they did.
- The user's data folder holds a copy of each plugin used, kept between starts; earlier copies from the same place are removed when replaced.
- Every plugin repository repacks its release files (`build-oxp.sh` packs with the system's `tar`), and every OxiGuitar from 0.23 reads only archives: plugins must be released as archives before the core release that needs them, and the core's tests repack released bare libraries until they are.
- Putting a newer version in the user's folder no longer updates a plugin that comes with a portable copy, the AppImage or the macOS app: that copy comes from the program's own folder, updated with the program.
