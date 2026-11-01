# Installing on macOS

OxiGuitar for macOS is a .dmg: open it and drag OxiGuitar to Applications. It runs on Intel and Apple silicon. [Installing](installing.md) lists the files and the first-launch steps. This page is about plugins.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `~/Library/Application Support/OxiGuitar/plugins`, and inside the app, in `Contents/PlugIns`. **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found. [The official plugins](official-plugins.md) says what each does.

## Today

**The app carries the basic plugins** inside itself, so Guitar Pro files open and play at once.

## Coming with OxiGuitar 1.0

OxiGuitar 1.0 splits the plugins from the program. Nothing below is published yet; the exact commands will be here when it is.

- **Homebrew, through our own tap.** `brew install` of OxiGuitar and of each plugin by name, and `brew upgrade` keeps them all current. Each plugin's package installs its file into the folder above, and the optional ones can be added any time.
- **An installer (.pkg) for each plugin,** for those without Homebrew. It installs for you alone, in your own Library, without asking for a password.
- The app then holds the program only. You need at least the Guitar Pro plugin, the MIDI plugin and the Core Audio plugin (the one that sends sound to your speakers).

### Why macOS asks about OxiGuitar

OxiGuitar is signed without an Apple developer account (the project doesn't pay Apple for one), so macOS shows its warning the first time. See [Installing](installing.md) for **Open Anyway** in System Settings. A plugin that came in a download and is refused is cleared the same way: in Terminal,

```sh
xattr -d com.apple.quarantine ~/Library/Application\ Support/OxiGuitar/plugins/*.oxp
```

A plugin installed by a .pkg isn't marked this way, so it needs no clearing. Whether a plugin installed by Homebrew needs it will be tested before the tap is published, and this page will say.

If a plugin is missing, OxiGuitar says so once when it starts and links here.
