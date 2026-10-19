# oxiguitar-plugin-test

The contract suite for [OxiGuitar](https://github.com/mateusrdp/oxiguitar) plugins. As a dev-dependency, `oxiguitar_plugin_test::build` builds your plugin as an `.oxp` file and `check` loads it through OxiGuitar's own plugin host and checks it against the interface; as a program (`cargo install oxiguitar-plugin-test`), `oxiguitar-plugin-test FILE.oxp` checks built files.

See "Testing" in [Writing OxiGuitar plugins](https://mateusrdp.github.io/oxiguitar/plugins/).

Licence: GPL-3.0-or-later. Used as a dev-dependency or a tool, it puts no licence on your plugin.
