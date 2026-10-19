# The SDK

The SDK is the crate `oxiguitar-plugin`. It wraps the plugin interface in safe Rust: you implement traits, and `export_plugin!` makes the C entry point OxiGuitar loads.

## The crate

```toml
{{#include ../template/Cargo.toml}}
```

Four things in it matter to OxiGuitar:

- **The `oxp` feature.** `export_plugin!` exports the entry point and the manifest's section only with it, so the same crate is an ordinary library for `cargo test` and a plugin file when built with `--features oxp --crate-type cdylib`. Without the feature in `Cargo.toml`, that build fails with "does not have the feature `oxp`".
- **`unsafe_code = "deny"`**, not `"forbid"`: the two attributes `export_plugin!` generates (`#[unsafe(no_mangle)]` and `#[unsafe(link_section)]`) need `unsafe`, and allow it for themselves only. You write no `unsafe` code.
- **`panic = "unwind"`**, Rust's default. A panic in a call is then caught and reported as an error; with `panic = "abort"` it closes OxiGuitar.
- **The SDK's version**: `0.2`, the release of OxiGuitar you build for. A plugin built with the SDK of 0.2 loads in every later OxiGuitar with interface 1.x ([Interface versions](versions.md)).

## The plugin

```rust,ignore
{{#include ../template/src/lib.rs:plugin}}
```

`Plugin` has one required method and five optional ones:

| Method | Called when | Returns |
| --- | --- | --- |
| `start(host)` | The plugin is first needed | The plugin, or an error: it is then never called again |
| `reader(format)` | A file of the format `format` (an id from the manifest) is opened | A `FormatReader`, or `None` |
| `writer(format)` | A score is saved in the format | A `FormatWriter`, or `None` |
| `service(interface, id)` | The service is probed or called | A `Service`, or `None` |
| `tab(id)` | The tab is shown or used | A `Tab`, or `None` |
| `event(event)` | Something the manifest's `events` subscribe to happened | Nothing |

The ids are the manifest's. Returning `None` answers "not found", which the contract suite checks for a contribution the plugin doesn't have; `cargo test` checks that every contribution the manifest declares is found.

## Threads

Every call may come from any thread, several at once, so `Plugin` and every contribution are `Send + Sync`, and every method takes `&self`. Keep what changes behind a `Mutex` or in atomics, and don't hold a lock while you call the host or do slow work: the template's tab does

```rust,ignore
{{#include ../template/src/statistics.rs:state}}
```

and takes the lock only to read or change `State`.

## The host

`start` receives a `Host`: the core's services, as the plugin reaches them. Keep a clone wherever you need it (it is cheap to clone).

| Method | What it does | Since |
| --- | --- | --- |
| `notify(level, text)` | Shows a message in the footer, with the plugin's name. `Level::Info`, `Warning` or `Error` | 1.0 |
| `settings()` | The plugin's settings: the JSON object under its id in settings.json ([Settings](settings.md)) | 1.0 |
| `set_settings(map)` | Replaces them and saves settings.json | 1.0 |
| `call_service(interface, method, input)` | Calls the service OxiGuitar chose for `interface`, which the manifest must list under `requires` ([Services](services.md)) | 1.0 |
| `redraw(tab)` | Asks the core to show one of the plugin's tabs again ([Tabs](tabs.md)) | 1.1 |
| `score()` | The open score, or `None` ([Events](events.md)) | 1.1 |

`Host::detached()` answers like a host with no window, no settings and no services: for unit tests.

## Errors

A call that fails returns an `Error`: a kind, one sentence for the user, and where it happened.

```rust,ignore
Error {
    offset: Some(1032),            // the byte in the input, if there is one
    context: "bar 12, track 2".into(),
    ..Error::new(ErrorKind::Invalid, "a note is on string 9 of a six-string guitar")
}
```

| Kind | For |
| --- | --- |
| `NotFound` | No such contribution, method or widget: `Error::not_found(what)` |
| `Unsupported` | Input of a kind or version the plugin can't handle (a newer file version) |
| `Invalid` | Input that is damaged or breaks its rules |
| `UnexpectedEnd` | Input that ends before the data it promises |
| `Failed` | Anything else |

When a file can't be read, the footer says `FILE: message (in context) at byte offset`; a crash or a malformed answer is named with your plugin's name instead. Write the message for a musician, not a programmer: "the file ends in the middle of bar 12", not "unexpected EOF in read_u32".

## Built in, and in tests

Without the `oxp` feature the crate is an ordinary library, and `oxiguitar_plugin::dispatch(&plugin, request)` answers a request the way the plugin file would, messages and all. Our own tests use plugins that way, and so will the mobile apps, which build their plugins in.
