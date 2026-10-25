# Testing

## cargo test

In the template, `cargo test` runs three kinds of test:

1. **Your unit tests**, on the plugin as an ordinary Rust library: call your reader on sample files, your tab's `on` and `content`, your service's `call`, with `Host::detached()` where a host is needed. The template's are in each module of `src/`.
2. **`the_manifest_matches_the_plugin`**, which `export_plugin!` adds: the manifest is valid and has no unknown key, its interface is one the SDK has, its version is the crate's, every declared format, service, tab, tool, settings window and track section is implemented, and the first contents of every tab, settings window and track section can be shown.
3. **The contract suite**, in `tests/contract.rs`:

   ```rust,ignore
   {{#include ../template/tests/contract.rs:contract}}
   ```

   It builds the crate as `target/oxp/example.oxp`, copies it into a plugin folder of its own, and checks it through OxiGuitar's own plugin host, as the app loads it:

   - the manifest reads without loading the plugin;
   - it loads and starts;
   - a contribution it doesn't have is "not found", not a crash;
   - every format it reads answers an empty file, a kilobyte of zeros and its own signature alone with a score or an error, never a crash;
   - every format it writes answers an empty score;
   - every service answers its probe;
   - every tab's contents can be shown, before and after a plausible use of each widget;
   - every kind of event it subscribes to is taken without an error;
   - every tool runs with no score open;
   - its settings window and its track sections show contents that can be shown, before and after a plausible use of each widget, and a section hands back its track's settings as a JSON object;
   - every instrument and effect prepares; an instrument's note starts within its resolution and not before; neither allocates while rendering, and on Linux neither makes a system call ([Instruments](instruments.md)).

A test that loads a plugin file must build it once per test program and never rebuild it while it is loaded: a system loads a library file once per process (and Windows refuses to replace a loaded file). One contract test per crate, as in the template, keeps to that.

## oxiguitar-plugin-test

The contract suite is also a program, for plugin files you have built:

```sh
cargo install --git https://github.com/mateusrdp/oxiguitar --tag v0.5.0 --locked oxiguitar-plugin-test
oxiguitar-plugin-test target/oxp/example.oxp
oxiguitar-plugin-test --build example-plugin example   # build PACKAGE as NAME.oxp, then check it
```

It prints every check, passed (`ok`) or not (`FAIL`, with why), every footer message the plugin sent, and how long its manifest took to read, and exits with an error if any check failed. Run it on each system's file in CI: the template's workflow does it through `cargo test`.

## oxiguitar-cli

`oxiguitar-cli` opens files through plugins exactly as the window does, without a window: quicker than the app for a format plugin.

```sh
oxiguitar-cli detect song.extab                  # which format and plugin recognise it
oxiguitar-cli info song.extab                    # the score's outline, the bar check and the import log
oxiguitar-cli info --bars song.extab             # and every bar of every track
oxiguitar-cli render song.extab --png page       # page-1.png, … as the window engraves it
```

It comes with the Linux packages and the Windows zip; elsewhere, `cargo install --git https://github.com/mateusrdp/oxiguitar --tag v0.5.0 --locked oxiguitar-cli`. It reads the same plugin folders as the app, so put your file in a plugin folder first (below).

## In the app

Try your plugin in a throwaway OxiGuitar home rather than your own, so a mistake can't touch your settings: `OXIGUITAR_HOME` puts settings.json and the plugin folder in one folder.

```sh
mkdir -p /tmp/og/plugins
cp target/oxp/example.oxp /tmp/og/plugins/
OXIGUITAR_HOME=/tmp/og oxiguitar samples/open-strings.extab
```

(On Windows: `set OXIGUITAR_HOME=C:\temp\og` before `oxiguitar.exe`.) Then:

- **Tools → Plugins** lists every plugin file found, with its state: loaded, not loaded yet, replaced by another file of the same plugin, or why it can't be used (a manifest problem, another system's file, a newer interface, conditions that don't hold). A plugin is loaded the first time one of its contributions is used.
- **The footer** (the line at the bottom; click it for the session's messages) shows your warning and error `notify` messages, errors from your calls, and, if your plugin fails, the line saying it was withdrawn and why (a crash's panic message, say). It isn't a log: `Level::Info` messages and import logs don't show there; `oxiguitar-cli info` prints a file's import log.
- **`oxiguitar --safe-mode`** starts with no plugin loaded at all, while Tools → Plugins still lists them and can disable one: the way out when a plugin breaks the app.
- **Help → About** lists the plugin folders in the order they are searched.

The app loads a plugin file once: after building it again, copy the new file over and restart OxiGuitar.

## Fuzzing

A reader of a binary format should be fuzzed, so that no file can crash it. With [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) (nightly Rust), a target is a few lines:

```rust,ignore
#![no_main]
use oxiguitar_plugin::FormatReader;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = example_plugin::format::ExampleTab.read(data);
});
```

Start it from a few real files of the format, and run it for hours rather than minutes; ours run six hours per reader before a release.
