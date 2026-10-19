# oxiguitar-plugin

The SDK for [OxiGuitar](https://github.com/mateusrdp/oxiguitar) plugins. A plugin implements `Plugin` and a trait for each thing it adds (`FormatReader`, `FormatWriter`, `Service`, `Tab`), writes its `manifest.json`, and calls `export_plugin!`; built with `--features oxp --crate-type cdylib`, the crate becomes a `.oxp` file OxiGuitar loads. Plugin authors write no `unsafe` code.

Start from the template and read the guide: [Writing OxiGuitar plugins](https://mateusrdp.github.io/oxiguitar/plugins/).

Licence: MIT OR Apache-2.0, so a plugin may use any GPL-compatible licence.
