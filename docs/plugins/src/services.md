# Services

A service is one implementation of an **interface**: a named set of methods that whoever defines it documents. Several plugins may implement the same interface for different systems or environments (an audio device through PipeWire, ALSA, CoreAudio or WASAPI, say), and OxiGuitar chooses one that works here. Plugins call services through the host, so a plugin can build on another without knowing which one is installed.

The core defines its interfaces as it needs them (the audio device comes with the player). A plugin may define its own, for other plugins: the template's `note-names` names a pitch.

## Implementing one

```json
"services": [
  { "interface": "note-names", "id": "example", "name": "Example note names",
    "priority": 0, "conditions": { "os": ["linux"] } }
]
```

| Key | What it is |
| --- | --- |
| `interface` | The interface it implements |
| `id` | Its id, unique for that interface in the plugin |
| `name` | Shown to the user |
| `priority` | Higher wins when several can run and settings.json pins none (default 0) |
| `conditions` | Where it can run, beyond the plugin's own conditions ([The manifest](manifest.md)) |

```rust,ignore
pub trait Service: Send + Sync {
    fn probe(&self) -> Availability { Availability::Available }
    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error>;
}
```

The template's:

```rust,ignore
{{#include ../template/src/service.rs:service}}
```

`probe` says whether the service can work here: a sound server is running, a device is present. Answer `Availability::Unavailable("PipeWire isn't running".into())` otherwise; the text is shown to the user when nothing else works.

`call` takes a method's name and its input as bytes, and answers with its output as bytes; what the bytes hold is the interface's to define. For anything beyond a few bytes, encode a Rust type with `oxiguitar_plugin::api::encode` and `decode` (postcard, as the interface's own messages use), and publish the type in a small crate both sides depend on.

## How OxiGuitar chooses

For an interface, the candidates are the services of every usable plugin whose conditions hold here.

1. If settings.json pins one, as `"services": {"note-names": "yourname.example/example"}` (`PLUGIN-ID/SERVICE-ID`), it is tried first.
2. Otherwise the highest `priority` comes first, then plugin order.
3. Each candidate's `probe` is asked in turn (which loads its plugin), and the first that says yes is used for the rest of the session.

The contract suite checks that every service you declare answers its probe.

## Calling one

List the interface under `requires` in the manifest, then call it through the host:

```json
"requires": ["note-names"]
```

```rust,ignore
let name = host.call_service("note-names", "name", &[61, 0])?;  // "C♯4"
```

A plugin may call only the interfaces it `requires`. When no installed plugin implements one, or none works here, the call returns an error saying so: handle it, and tell the user in your README which plugins yours needs.

## The audio device (interface 1.3)

`audio-device` is the core's own interface: the service that sends OxiGuitar's sound to the system. The basic plugins implement it for PipeWire and ALSA (Linux), WASAPI (Windows) and Core Audio (macOS), through cpal. Its types are in `oxiguitar_plugin::api::audio_device`.

| Method | Input | Output |
| --- | --- | --- |
| `open` | `Open { stream, rate, frames, device }` | `Opened { rate, channels, frames, device }` |
| `close` | the stream's number, a `u64` | nothing |
| `devices` | nothing | the outputs' names, a `Vec<String>` |

`open` opens a stream on the output named (empty: the system's default) at the rate asked for (48 000), or at the device's own when it refuses that one, and starts it. From then on, in the device's audio callback, the plugin has the core fill each buffer:

```rust,ignore
let out = host.audio_out().ok_or(...)?;  // None before interface 1.3
// In the callback, for every buffer:
out.render(stream, buffer, channels, delay_ns);
```

`buffer` is interleaved frames of `channels` channels, and `delay_ns` is how long until they reach the speaker (the device's latency), which the cursor follows. The callback is the audio thread: `render` never blocks, and your callback must not either. After `close` returns, never call `render` for that stream again. When a stream breaks (the device unplugged, the sound server gone), say so with `host.notify`.

Users pick a device by pinning it in settings.json, as `"services": {"audio-device": "oxiguitar.audio-alsa/alsa"}`. The test plugin's `fake` device (priority -100) renders into nothing, so the contract tests can play a whole stream without a sound card.
