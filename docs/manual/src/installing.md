# Installing

Download the one file for your system from the [releases page](https://github.com/mateusrdp/oxiguitar/releases). Each exists for x86_64 (Intel, AMD) and ARM64 machines; the macOS app runs on both. Every download comes with the basic plugins, which open Guitar Pro files of every version from 3 to 8 and play them through your system's sound.

| System | Download | Install |
| --- | --- | --- |
| Any Linux | `OxiGuitar-VERSION-x86_64.AppImage` or `-aarch64` | `chmod +x OxiGuitar-*.AppImage`, then run it |
| Debian, Ubuntu | `oxiguitar_VERSION_amd64.deb` and `oxiguitar-plugins-basic_VERSION_amd64.deb` (or `_arm64`) | `sudo apt install ./oxiguitar_*.deb ./oxiguitar-plugins-basic_*.deb` |
| Fedora | `oxiguitar-VERSION-1.x86_64.rpm` and `oxiguitar-plugins-basic-VERSION-1.x86_64.rpm` (or `.aarch64`) | `sudo dnf install ./oxiguitar-*.rpm` |
| Windows 10 and 11 | `OxiGuitar-VERSION-windows-x86_64.zip` or `-arm64` | Unzip anywhere and run `oxiguitar.exe` |
| macOS | `OxiGuitar-VERSION-macos-universal.dmg` | Open it and drag OxiGuitar to Applications |

The AppImage needs glibc 2.35 or newer (Debian 12 or later, Ubuntu 22.04 or later, Fedora 36 or later); the .deb and .rpm need glibc 2.39 or newer (Debian trixie or sid, Ubuntu 24.04, Fedora 40 or later). The .deb and .rpm of the program depend on the plugins' package, so install the two together as shown. They also install `oxiguitar-cli`, a command-line tool that prints a file's contents, converts it to Guitar Pro 7's .gp, and engraves it to PNG and PDF (`oxiguitar-cli --help`), and both programs' manual pages (`man oxiguitar`, `man oxiguitar-cli`).

### Wayland and X11

OxiGuitar comes in two Linux builds: the **Wayland build**, for today's desktops (GNOME, KDE Plasma and most others run Wayland), and the **X11 build**, for desktops that still run only an X server. Each carries only its own display system's code.

- **The AppImage holds both** and starts the right one by itself: the Wayland build in a Wayland session, the X11 build otherwise. It plays through ALSA, which desktops with PipeWire pass on to PipeWire; the .deb and .rpm also have a PipeWire plugin, used first.
- **The .deb and .rpm above are the Wayland build.** For an X11 desktop, install the X11 build's package instead, with the same plugins' package: `oxiguitar-x11_VERSION_amd64.deb` (`sudo apt install ./oxiguitar-x11_*.deb ./oxiguitar-plugins-basic_*.deb`) or `oxiguitar-x11-VERSION-1.x86_64.rpm`. Installing one build's package replaces the other's.

If you start the wrong package's build, it says which one you need. **The X11 build is deprecated** and will be dropped once desktops have left X11.

## The plugins

OxiGuitar opens and plays files only through plugins, and the basic ones come with every download: in the AppImage, inside the Windows zip (its `plugins` folder) and inside the macOS app, and as the `oxiguitar-plugins-basic` package beside the .deb and .rpm. You don't need to install them yourself.

To add other plugins, put their `.oxp` files in your own plugin folder ([Settings and plugins](plugins.md)); when the same plugin is in both places, OxiGuitar uses the higher version, and yours when the versions are equal. **Help → About** lists every folder OxiGuitar searches, and **Tools → Plugins** the plugins it found. To leave out a plugin you don't want, untick it in **Tools → Plugins**. (Releases up to 0.10 came without plugins, which were a separate zip. If you unzipped one into your plugin folder, you can delete those files: the newer copies that come with OxiGuitar are used anyway.)

On macOS, if the system refuses a plugin you downloaded yourself ("cannot be opened because the developer cannot be verified"), clear the download mark on the plugin files and start OxiGuitar again:

```sh
xattr -d com.apple.quarantine ~/Library/Application\ Support/OxiGuitar/plugins/*.oxp
```

## The first launch

The packages are not code-signed yet, so the system warns the first time:

- **Windows:** SmartScreen says "Windows protected your PC". Click **More info**, then **Run anyway**.
- **macOS:** Control-click the app, choose **Open**, then **Open** again. On recent macOS you may need **System Settings → Privacy & Security → Open Anyway** instead.

## Opening Guitar Pro files with a double click

The .deb, .rpm and .dmg register the Guitar Pro file types (.gp, .gpx, .gp5, .gp4, .gp3), so double-clicking such a file opens it in OxiGuitar. The portable Windows .exe and the AppImage change nothing on your system, so you register them by hand if you want to.

**Windows.** Right-click a .gp file, choose **Open with → Choose another app**, then **Choose an app on your PC** (or **More apps → Look for another app on this PC**), pick `oxiguitar.exe` and tick **Always use this app**. Repeat for .gpx, .gp5, .gp4 and .gp3.

**AppImage.** Put the AppImage where it will stay, then run, with its path in the first line:

```sh
app="$HOME/Applications/OxiGuitar-x86_64.AppImage"
raw=https://raw.githubusercontent.com/mateusrdp/oxiguitar/main
id=io.github.mateusrdp.OxiGuitar
data="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$data/applications" "$data/mime/packages" "$data/icons/hicolor/256x256/apps"
curl -fsSL "$raw/assets/linux/$id.xml" -o "$data/mime/packages/$id.xml"
curl -fsSL "$raw/assets/icon/png/oxiguitar-256.png" -o "$data/icons/hicolor/256x256/apps/$id.png"
curl -fsSL "$raw/assets/linux/$id.desktop" | sed "s|^Exec=oxiguitar |Exec=\"$app\" |" > "$data/applications/$id.desktop"
update-mime-database "$data/mime"
xdg-mime default "$id.desktop" application/x-gtp
```

This also gives OxiGuitar its icon and an entry in your desktop's menu. To undo it, delete the three files and run `update-mime-database` again. With a copy of the source code, `scripts/install-desktop-entry.sh PATH-TO-APPIMAGE` does the same.

**macOS.** Double-clicking a Guitar Pro file starts OxiGuitar but does not open the file: open it with **File → Open**, or drop it on the window.
