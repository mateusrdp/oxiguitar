# Installing

Download the package for your system from the [releases page](https://github.com/mateusrdp/oxiguitar/releases). Each package exists for x86_64 (Intel, AMD) and ARM64 machines; the macOS app runs on both.

| System | Package | Install |
| --- | --- | --- |
| Debian, Ubuntu | `oxiguitar_VERSION_amd64.deb` or `_arm64.deb` | `sudo apt install ./oxiguitar_*.deb` |
| Fedora | `oxiguitar-VERSION-1.x86_64.rpm` or `.aarch64.rpm` | `sudo dnf install ./oxiguitar-*.rpm` |
| Any Linux | `OxiGuitar-VERSION-x86_64.AppImage` or `-aarch64` | `chmod +x OxiGuitar-*.AppImage`, then run it |
| Windows 10 and 11 | `OxiGuitar-VERSION-windows-x86_64.zip` or `-arm64` | Unzip anywhere and run `oxiguitar.exe` |
| macOS | `OxiGuitar-VERSION-macos-universal.dmg` | Open it and drag OxiGuitar to Applications |

Linux needs glibc 2.39 or newer: Debian trixie or sid, Ubuntu 24.04, Fedora 40 or later. The .deb and .rpm also install `oxiguitar-cli`, a command-line tool that prints a file's contents and engraves it to PNG and PDF (`oxiguitar-cli --help`).

### Wayland and X11

The Linux packages above are the **Wayland build**, for today's desktops (GNOME, KDE Plasma and most others run Wayland). A desktop that still runs only an X server needs the **X11 build** instead:

| System | Package | Install |
| --- | --- | --- |
| Debian, Ubuntu | `oxiguitar-x11_VERSION_amd64.deb` or `_arm64.deb` | `sudo apt install ./oxiguitar-x11_*.deb` |
| Fedora | `oxiguitar-x11-VERSION-1.x86_64.rpm` or `.aarch64.rpm` | `sudo dnf install ./oxiguitar-x11-*.rpm` |
| Any Linux | `OxiGuitar-x11-VERSION-x86_64.AppImage` or `-aarch64` | `chmod +x OxiGuitar-x11-*.AppImage`, then run it |

It is the same program, and each build carries only its own display system's code. Installing one package replaces the other. If you start the wrong one, it says which build you need. **The X11 build is deprecated** and will be dropped once desktops have left X11.

## The plugins

The packages above hold the program alone, and it opens files only through plugins. Install the **basic plugins** too, which open Guitar Pro files of every version from 3 to 8:

| System | Package | Install |
| --- | --- | --- |
| Debian, Ubuntu | `oxiguitar-plugins-basic_VERSION_amd64.deb` or `_arm64.deb` | `sudo apt install ./oxiguitar-plugins-basic_*.deb` |
| Fedora | `oxiguitar-plugins-basic-VERSION-1.x86_64.rpm` or `.aarch64.rpm` | `sudo dnf install ./oxiguitar-plugins-basic-*.rpm` |
| Any Linux (AppImage) | `OxiGuitar-plugins-basic-VERSION-linux-x86_64.zip` or `-aarch64` | Unzip the `.oxp` files into `~/.local/share/oxiguitar/plugins/` |
| Windows | `OxiGuitar-plugins-basic-VERSION-windows-x86_64.zip` or `-arm64` | Unzip the `.oxp` files into `%APPDATA%\OxiGuitar\plugins\`, or into a `plugins` folder beside `oxiguitar.exe` |
| macOS | `OxiGuitar-plugins-basic-VERSION-macos-universal.zip` | Unzip the `.oxp` files into `~/Library/Application Support/OxiGuitar/plugins/` |

The .deb and .rpm work with either Linux build. OxiGuitar creates your plugin folder the first time it starts; **Help → About** lists every folder it searches, and **Tools → Plugins** the plugins it found. Each plugin is also attached to the release on its own (`oxiguitar-gp5-VERSION-linux-x86_64.oxp` …), if you want only some. Until OxiGuitar has a plugin, its footer says that nothing can be opened yet.

On macOS, if the system refuses a plugin it downloaded ("cannot be opened because the developer cannot be verified"), clear the download mark on the plugin files and start OxiGuitar again:

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

**macOS.** Double-clicking a Guitar Pro file starts OxiGuitar, but version 0.1 does not open the file yet: open it with **File → Open**.
