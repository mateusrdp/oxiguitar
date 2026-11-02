# Installing on macOS

OxiGuitar for macOS is a .dmg: open it and drag OxiGuitar to Applications. It runs on Intel and Apple silicon. [Installing](installing.md) lists the files and the first-launch steps. This page is about plugins.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `~/Library/Application Support/OxiGuitar/plugins`, and inside the app, in `Contents/PlugIns` (empty: the app holds no plugin). **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found. [The official plugins](official-plugins.md) says what each does.

## Plugins

The app holds the program only. Install the plugins either way:

- **Homebrew, through our own tap:**

  ```sh
  brew tap mateusrdp/oxiguitar
  brew install --cask oxiguitar-guitarpro oxiguitar-midi
  ```

  Add `oxiguitar-wav`, `oxiguitar-nam` (which needs `oxiguitar-midi`, and installs it) and `oxiguitar-coreaudio` the same way, and `brew upgrade` keeps them current. Each plugin's cask puts its file in the folder above and clears macOS's quarantine mark.
- **An installer (.pkg) for each plugin,** from its releases page, for those without Homebrew. It installs for you alone, in your own Library, without asking for a password.

You need at least the Guitar Pro plugin, the MIDI plugin and the Core Audio plugin (the one that sends sound to your speakers).

### Why macOS asks about OxiGuitar

OxiGuitar is signed without an Apple developer account (the project doesn't pay Apple for one), so macOS shows its warning the first time. See [Installing](installing.md) for **Open Anyway** in System Settings. A plugin that came in a download and is refused is cleared the same way: in Terminal,

```sh
xattr -d com.apple.quarantine ~/Library/Application\ Support/OxiGuitar/plugins/*.oxp
```

A plugin installed by a .pkg isn't marked this way, so it needs no clearing. A plugin installed by Homebrew is cleared by its cask.

If a plugin is missing, OxiGuitar says so once when it starts and links here.
