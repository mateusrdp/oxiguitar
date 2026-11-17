# 0011 — Our code never looks at the environment to choose what to do; libraries are theirs, and others' plugins may but shouldn't

- Status: accepted (Mateus, 17 Nov 2026)
- Date: 2026-11-17
- Milestone: v1.0 (issue #271)
- Supersedes: ADR 0009's rule that the Wayland build holds no X11 crate, and the Slint pin that followed from it

## Context

ADR 0009 made the Wayland build hold no X11 crate at all, libraries' crates included, and pinned Slint to 1.17.1 to keep it so. The pin was a constant obstacle (#271).

Slint 1.19.0 (16 Nov 2026) puts the Wayland clipboard back on the window's own connection (smithay-clipboard), which works on every compositor, GNOME included: the Wayland path ADR 0009 wanted. But its `wayland` feature still compiles arboard as the clipboard for a display that isn't Wayland, so `x11rb` is in the Wayland build's tree, though that build never reaches it. Under ADR 0009 that kept the pin.

What ADR 0009 was guarding against is a hack in our own code: detecting the environment and branching ("if Wayland do this, else do that"). What a library carries inside is not something we write, maintain or can change.

## Decision

Mateus (17 Nov 2026):

- **Our code never looks at the environment to choose what to do.** No "if Wayland do this, else do that" in the app or in our own plugins. What differs between platforms or display systems is chosen when building (Cargo features, `cfg`): each build knows what it is. This is a directive of ours (CONTRIBUTING.md, "Design principles").
- **If one of our plugins depends on the environment, it is handled in that plugin, never in the app.**
- **Libraries are theirs.** We fully delegate to libraries: their dependencies, fallbacks and code paths are not ours to audit, trim or work around.
- **Other people's plugins may, but shouldn't.** A plugin may carry code it never uses or look at the environment to choose what to do. We discourage it, and the plugin guide says so, but OxiGuitar doesn't check for it: we don't limit plugin developers. That they can doesn't mean they should.
- **Code meant to run that a library's bug stops is not a breach.** The file drop in the window is written for every platform and should work everywhere; on Wayland it doesn't, because winit 0.30 sends no drops there (ADR 0006). It stays as it is.
- Slint follows the latest release again: 1.19.0 now.

ADR 0009's other parts stand: two Linux builds, Wayland first (when they pull different ways the Wayland build gets the clean, native solution), the X11 build deprecated until it is dropped.

## Consequences

- The app's display system, which plugins' `display` conditions are checked against, is a constant of its build (`DISPLAY_SYSTEM` in `oxiguitar-app`). A build with both the `wayland` and `x11` features doesn't compile. The host no longer guesses from `WAYLAND_DISPLAY`: programs that draw nothing (the CLI, tests) say `oxiguitar_host::NO_DISPLAY`, so a plugin whose conditions name display systems is not used by them. None of our plugins names one.
- `scripts/check-display-crates.sh` no longer lists libraries' crates. It checks our own choices: each Linux build turns on its own windowing backend in Slint and winit, and not the other one; and no tracked Rust file of ours names a session variable (`WAYLAND_DISPLAY`, `DISPLAY`, `XDG_SESSION_TYPE` and the like) as a string.
- The Wayland build compiles arboard and `x11rb` inside Slint's winit backend. They never run there: on Wayland, Slint uses smithay-clipboard on the window's connection.
- The AppImage's `AppRun` and the Flatpak's `oxiguitar-start` are launchers, not the app: they pick which of the two programs to start from the session, because those packages carry both builds.
- `deny.toml` loses the exceptions for Slint 1.17.1's crates (`paste`, `rustybuzz`).
- A Slint upgrade is now an ordinary dependency upgrade: read its changelog, build, test, and check the window on Wayland and X11.
- Plugin repositories that call `oxiguitar_host::display_system()` in their tests (oxiguitar-guitarpro's `crates/oxiguitar-formats/tests/host.rs`) use `oxiguitar_host::NO_DISPLAY` when they move to a core release with this change.
