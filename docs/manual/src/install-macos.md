# Installing on macOS

OxiGuitar for macOS is a .dmg: open it and drag OxiGuitar to Applications. It runs on Intel and Apple silicon. [Installing](installing.md) lists the files and the first-launch steps. This page is about plugins.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `~/Library/Application Support/OxiGuitar/plugins`, and inside the app, in `Contents/PlugIns` (empty: the app holds no plugin). **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found. [The official plugins](official-plugins.md) says what each does.

## Plugins

The app holds the program only. Install each plugin with its installer:

- **An installer (.pkg) for each plugin,** from its releases page. It installs for you alone, in your own Library, without asking for a password.

You need at least the Guitar Pro plugin, the MIDI plugin and the Core Audio plugin (the one that sends sound to your speakers).

For the free recorded sound sets we recommend, the sound-set installer's releases page has `install.sh`: run it in Terminal (`bash install.sh`); it lists what it would download, from where and under which licence, and asks before it fetches anything, into your `soundfonts` folder ([Better sounds](better-sounds.md#the-sound-set-installer)).

### Why macOS asks about OxiGuitar

OxiGuitar is signed without an Apple developer account (the project doesn't pay Apple for one), so macOS shows its warning the first time. See [Installing](installing.md) for **Open Anyway** in System Settings. A plugin that came in a download and is refused is cleared the same way: in Terminal,

```sh
xattr -d com.apple.quarantine ~/Library/Application\ Support/OxiGuitar/plugins/*.oxp
```

A plugin installed by a .pkg isn't marked this way, so it needs no clearing.

If a plugin is missing, OxiGuitar says so once when it starts and links here.
