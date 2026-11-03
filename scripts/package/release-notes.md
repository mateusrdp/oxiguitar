## Which file do I need?

One download for your system. **None carries plugins**: the plugins that open Guitar Pro files and play them have releases and packages of their own ([oxiguitar-guitarpro](https://github.com/mateusrdp/oxiguitar-guitarpro), [oxiguitar-midi](https://github.com/mateusrdp/oxiguitar-midi) and the others). The package repositories and the Homebrew tap bring them with OxiGuitar and keep them up to date; [Installing](https://mateusrdp.github.io/oxiguitar/installing.html) explains each way.

| System | Download |
| --- | --- |
| Windows 10 and 11 (x86_64) | `OxiGuitar-@VERSION@-windows-x86_64.zip` |
| Windows on ARM | `OxiGuitar-@VERSION@-windows-arm64.zip` |
| macOS (Apple Silicon and Intel) | `OxiGuitar-@VERSION@-macos-universal.dmg` |
| Any Linux (x86_64; `aarch64` for ARM) | `OxiGuitar-@VERSION@-x86_64.AppImage`, for Wayland and X11 desktops alike |
| Debian, Ubuntu (x86_64; `arm64` for ARM) | `oxiguitar_@VERSION@_amd64.deb`, or `apt install oxiguitar` from the apt repository |
| Fedora (x86_64; `aarch64` for ARM) | `oxiguitar-@VERSION@-1.x86_64.rpm`, or `dnf install oxiguitar` from the dnf repository |

The .deb and .rpm above are the Wayland build; a Linux desktop without Wayland takes the files named `oxiguitar-x11` instead. SHA256SUMS holds every file's checksum.
