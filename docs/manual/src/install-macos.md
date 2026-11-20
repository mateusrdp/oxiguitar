# Installing on macOS

OxiGuitar for macOS is a .dmg: open it and drag OxiGuitar to Applications. It runs on Intel and Apple silicon. [Installing](installing.md) lists the files and the first-launch steps. This page is about plugins.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `~/Library/Application Support/OxiGuitar/plugins`, and inside the app, in `Contents/PlugIns` (empty: the app holds no plugin). **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found.

## Plugins

The app holds the program only. Each plugin has an installer (.pkg) on its releases page. It installs for you alone, in your own Library, without asking for a password.

You need at least a plugin that reads Guitar Pro files, one that plays a score and the Core Audio plugin, which sends the sound to your speakers. This work depends on the [packages manual](https://mateusrdp.github.io/oxiguitar-packages/manual/plugins.html), which links to the manual and releases page of every plugin.

### Why macOS asks about OxiGuitar

OxiGuitar is signed without an Apple developer account (the project doesn't pay Apple for one), so macOS shows its warning the first time. See [Installing](installing.md) for **Open Anyway** in System Settings. A plugin that came in a download and is refused is cleared the same way: in Terminal,

```sh
xattr -d com.apple.quarantine ~/Library/Application\ Support/OxiGuitar/plugins/*.oxp
```

A plugin installed by a .pkg isn't marked this way, so it needs no clearing.

If a plugin is missing, OxiGuitar says so once when it starts and links here.
