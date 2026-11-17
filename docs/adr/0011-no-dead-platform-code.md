# 0011 — Our code holds no code that never runs where it is built; libraries are theirs

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

- **OxiGuitar's own code never holds code that never runs on the platform or build it is compiled for.** Each build compiles only the paths it can take. The choice is made when building (Cargo features, `cfg`), never by detecting the environment at run time and branching.
- **We don't care what libraries do: we fully delegate to them.** Their dependencies, their fallbacks and the code paths they hold are theirs. We don't audit, trim, patch or work around them for this rule.
- Slint follows the latest release again: 1.19.0 now.

ADR 0009's other parts stand: two Linux builds, Wayland first (when they pull different ways the Wayland build gets the clean, native solution), the X11 build deprecated until it is dropped.

## Consequences

- `scripts/check-display-crates.sh` no longer lists libraries' crates. It checks our own choice: each Linux build turns on its own windowing backend in Slint and winit, and not the other one.
- The Wayland build compiles arboard and `x11rb` inside Slint's winit backend. They never run there: on Wayland, Slint uses smithay-clipboard on the window's connection.
- `deny.toml` loses the exceptions for Slint 1.17.1's crates (`paste`, `rustybuzz`, `pin-utils`).
- A Slint upgrade is now an ordinary dependency upgrade: read its changelog, build, test, and check the window on Wayland and X11.
- Code of ours that this rule catches is brought in line, or decided on by Mateus, as it is found.
