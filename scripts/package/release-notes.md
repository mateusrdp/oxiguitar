## Which files do I need?

Two downloads: **the program** for your system, and **the plugins** it opens files with (without them OxiGuitar opens nothing).

| System | The program | The plugins |
| --- | --- | --- |
| Windows 10 and 11 (x86_64) | `OxiGuitar-@VERSION@-windows-x86_64.zip` | `OxiGuitar-plugins-basic-@VERSION@-windows-x86_64.zip` |
| Windows on ARM | `OxiGuitar-@VERSION@-windows-arm64.zip` | `OxiGuitar-plugins-basic-@VERSION@-windows-arm64.zip` |
| macOS (Apple Silicon and Intel) | `OxiGuitar-@VERSION@-macos-universal.dmg` | `OxiGuitar-plugins-basic-@VERSION@-macos-universal.zip` |
| Debian, Ubuntu (x86_64; `arm64` for ARM) | `oxiguitar_@VERSION@_amd64.deb` | `oxiguitar-plugins-basic_@VERSION@_amd64.deb` |
| Fedora (x86_64; `aarch64` for ARM) | `oxiguitar-@VERSION@-1.x86_64.rpm` | `oxiguitar-plugins-basic-@VERSION@-1.x86_64.rpm` |
| Any other Linux (x86_64; `aarch64` for ARM) | `OxiGuitar-@VERSION@-x86_64.AppImage` | `OxiGuitar-plugins-basic-@VERSION@-linux-x86_64.zip` |

Unzip the plugins into your plugin folder ([where it is](https://mateusrdp.github.io/oxiguitar/plugins.html)); the .deb and .rpm put them in place themselves. A Linux desktop without Wayland needs the X11 build instead of the program above: the files named `oxiguitar-x11` or `OxiGuitar-x11`. [Installing](https://mateusrdp.github.io/oxiguitar/installing.html) explains each step, and SHA256SUMS holds every file's checksum.

