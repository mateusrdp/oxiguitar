#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
"""Packs each plugin file in the folders given that is a bare library, as
plugin files were before OxiGuitar 0.23, into a tar archive (#347), in
place: manifest.json (the library's manifest section) first, the library
second as plugin.so, plugin.dll or plugin.dylib. Archives and other files
are left as they are.

Only until every plugin repository has released archives:
scripts/released-plugins.sh runs it on the releases it takes, which the
core's tests and workflows use.

Usage: scripts/repack-plugins.py FOLDER...
"""

import io
import os
import struct
import sys
import tarfile

ELF_SECTION = b"oxg_manifest"
PE_SECTION = b"oxgmanif"


def elf(data):
    little = data[5] == 1
    e = "<" if little else ">"
    if data[4] == 2:
        shoff, = struct.unpack_from(e + "Q", data, 0x28)
        shentsize, shnum, shstrndx = struct.unpack_from(e + "HHH", data, 0x3A)
        layout = e + "IIQQQQIIQQ"
    else:
        shoff, = struct.unpack_from(e + "I", data, 0x20)
        shentsize, shnum, shstrndx = struct.unpack_from(e + "HHH", data, 0x2E)
        layout = e + "IIIIIIIIII"
    headers = [struct.unpack_from(layout, data, shoff + i * shentsize) for i in range(shnum)]
    names = headers[shstrndx]
    for h in headers:
        start = names[4] + h[0]
        name = data[start:data.index(b"\0", start)]
        if name == ELF_SECTION:
            return data[h[4]:h[4] + h[5]]
    return None


def pe(data):
    at, = struct.unpack_from("<I", data, 0x3C)
    if data[at:at + 4] != b"PE\0\0":
        return None
    count, = struct.unpack_from("<H", data, at + 6)
    optional, = struct.unpack_from("<H", data, at + 20)
    table = at + 24 + optional
    for i in range(count):
        name, vsize, _, rawsize, rawptr = struct.unpack_from("<8sIIII", data, table + 40 * i)
        if name.rstrip(b"\0") == PE_SECTION:
            return data[rawptr:rawptr + min(vsize, rawsize)]
    return None


def macho(data, base=0):
    magic = data[base:base + 4]
    if magic == b"\xca\xfe\xba\xbe":
        # A universal file: every part carries the same manifest.
        count, = struct.unpack_from(">I", data, base + 4)
        for i in range(count):
            _, _, offset, size, _ = struct.unpack_from(">iiIII", data, base + 8 + 20 * i)
            found = macho(data, offset)
            if found is not None:
                return found
        return None
    if magic != b"\xcf\xfa\xed\xfe":
        return None
    ncmds, = struct.unpack_from("<I", data, base + 16)
    at = base + 32
    for _ in range(ncmds):
        cmd, size = struct.unpack_from("<II", data, at)
        if cmd == 0x19:  # LC_SEGMENT_64
            nsects, = struct.unpack_from("<I", data, at + 64)
            for s in range(nsects):
                sect = at + 72 + 80 * s
                name = data[sect:sect + 16].rstrip(b"\0")
                length, = struct.unpack_from("<Q", data, sect + 40)
                offset, = struct.unpack_from("<I", data, sect + 48)
                if name == ELF_SECTION:
                    return data[base + offset:base + offset + length]
        at += size
    return None


def manifest_and_name(data):
    """The library's manifest section and its name in a plugin file, or
    None for a file that isn't a plugin library."""
    if data[:4] == b"\x7fELF":
        return elf(data), "plugin.so"
    if data[:2] == b"MZ":
        return pe(data), "plugin.dll"
    if data[:4] in (b"\xca\xfe\xba\xbe", b"\xcf\xfa\xed\xfe"):
        return macho(data), "plugin.dylib"
    return None, None


def repack(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        manifest, name = manifest_and_name(data)
    except (struct.error, ValueError, IndexError):
        manifest, name = None, None
    if manifest is None:
        return False
    manifest = manifest.rstrip(b"\0")
    part = path + ".part"
    with tarfile.open(part, "w", format=tarfile.USTAR_FORMAT) as tar:
        for member, contents, mode in (("manifest.json", manifest, 0o644), (name, data, 0o755)):
            info = tarfile.TarInfo(member)
            info.size = len(contents)
            info.mode = mode
            tar.addfile(info, io.BytesIO(contents))
    os.replace(part, path)
    return True


def main(folders):
    if not folders:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    for folder in folders:
        for entry in sorted(os.listdir(folder)):
            path = os.path.join(folder, entry)
            if entry.endswith(".oxp") and os.path.isfile(path) and repack(path):
                print("repacked %s as an archive" % entry, file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
