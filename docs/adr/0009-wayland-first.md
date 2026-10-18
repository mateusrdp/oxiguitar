# 0009 — Wayland first: the Wayland build takes the Wayland path, and Slint is pinned to 1.17.1

- Status: accepted (revisit on every Slint upgrade)
- Date: 2026-10-19
- Milestone: M2 (slice 4)

## Context

M2's slice 4 splits the Linux core into two builds (ADR 0008, answer 9): a Wayland build, the default and the main one, and an X11 build, deprecated from the start. Each was meant to carry only its own display system's code.

Slint 1.18, which the app used, broke that for the Wayland build. Its winit backend replaced the clipboard it had used up to 1.17 (copypasta, which on Wayland is smithay-clipboard on the window's own Wayland connection, the protocol every compositor supports) with arboard. arboard always depends on X11 code (`x11rb`). On Wayland it uses the data-control protocol, which is meant for clipboard managers. GNOME's Mutter implements neither version of that protocol, so there arboard reaches the clipboard through XWayland. No arboard release makes the X11 part optional (3.7.0 is the newest, 15 Oct 2026). The first version of slice 4 listed this as the one exception to "no X11 crate in the Wayland build".

Mateus decided (19 Oct 2026): **always favour Wayland over X11 and take the Wayland favourable path. The X11 build gets workarounds until it is finally dropped.**

## Options for the clipboard

1. **Keep Slint 1.18** and its X11 fallback in the Wayland build. That is not the Wayland path.
2. **Replace Slint's clipboard ourselves** with smithay-clipboard on the window's connection. Slint's `Platform` would need a wrapper that forwards Slint's internal methods, and `smithay_clipboard::Clipboard::new` takes the display as a raw pointer, which means `unsafe` outside the loader and the SDK glue. It would also leave arboard in the build, since Slint still creates it. Rejected: a hack, and it doesn't remove the X11 code.
3. **Use the data-control protocol only** (wl-clipboard-rs, no X11). On GNOME the app would then have no system clipboard. Rejected.
4. **Pin Slint to 1.17.1**, whose Wayland build carries smithay-clipboard and no X11 crate at all. 1.17.1 is also the Slint Debian unstable has (`rust-i-slint-common` 1.17.1), which matters for Debian main (ADR 0008, answer 5).

## Decision

Option 4. `oxiguitar-app` pins `slint`, `slint-build` and `i-slint-backend-winit` to `=1.17.1`.

- The app needed one change for 1.17: ScrollView's scroll properties are `viewport-*` there (1.18 renamed them `content-*`).
- Slint 1.17 draws an image at a fractional position with its edges blended into what is behind it, so the score's tiles showed a faint line between them. 1.18 didn't. The tile layer now sits on whole device pixels: the page's origin is rounded, and the tiles are shifted by the fraction in the scroll position. This is right whatever Slint does. It was checked under Weston and Xvfb, before and after zooming.
- `scripts/check-display-crates.sh` allows **no** X11 crate in the Wayland build, arboard included, and no X11 support in the keyboard-map crate. A Slint upgrade that brings X11 back into the Wayland build fails CI.

## The rule for later choices

When the two builds pull different ways, the Wayland build gets the clean, native solution. A fallback, compatibility layer or workaround belongs in the X11 build, which will be dropped. Tests, examples and documentation use Wayland first (headless Weston), then X11 (Xvfb).

## What needs to change to move on from 1.17.1

A Slint release whose `backend-winit-wayland` holds no X11 crate: its winit backend using the window's own Wayland connection for the clipboard again, as copypasta did, or arboard making X11 optional. On each Slint release, run `scripts/check-display-crates.sh` against it. Raising it with Slint's developers is worth doing; the report is the facts above.

## Consequences

- No Slint 1.18 features. The app used none beyond the renamed properties, and the newer Slint's iOS work only matters at M10.
- Slint security or bug fixes after 1.17.1 need a look at each Slint release until the pin moves.
- Slint 1.17.1's code generator (`slint-build`, at build time only) takes the image crate with every format, AV1 encoder included (rav1e, without assembly), so a clean build takes a little longer. Two unmaintained crates come with 1.17.1 (`paste` at build time; `rustybuzz` in its SVG support). Neither is vulnerable, and `deny.toml` ignores both with this ADR as the reason. `deny.toml` now names our six targets, so dependencies no build of ours compiles (rav1e's `cfg(fuzzing)` libfuzzer-sys) no longer count.
- ADRs 0006 and 0007 were written against Slint 1.18. Their facts about winit 0.30 hold, since 1.17.1 uses the same winit.
