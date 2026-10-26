# OxiGuitar and Debian

OxiGuitar aims for Debian's main archive (ADR 0008, answer 5): both packages, `oxiguitar` and `oxiguitar-plugins-basic`, built by Debian from Debian's own packages. Debian builds a Rust program only from crates it packages itself, one `librust-NAME-dev` package per crate, so every crate in our tree has to be in Debian first.

`scripts/debian/crate-audit.py` checks that, and the Debian workflow (`.github/workflows/debian.yml`) runs it weekly and on every change to `Cargo.lock`. The report is the run's summary and notices on its check run.

## The first audit (19 Oct 2026, Debian sid)

424 crates: the app (both Linux builds), `oxiguitar-cli` and the five basic plugins, for x86_64 and aarch64, normal and build dependencies.

| Status | Crates | Meaning |
| --- | --- | --- |
| same | 276 | Debian has our version. |
| newer | 0 | Debian has a newer version of the same series. |
| older | 74 | Debian has an older version of the same series. Debian builds without our `Cargo.lock`, so these resolve to Debian's versions unless a `Cargo.toml` asks for more; most are patch releases. |
| other | 46 | Debian has the crate in another series only. |
| missing | 28 | Debian has no package of the crate. |

What stands in the way, by where it comes from:

- **Slint 1.17.1.** Debian has started on it (`i-slint-common`, `i-slint-core-macros`, `vtable`, `const-field-offset` are in sid at 1.17.1) but not `slint`, `slint-build`, `slint-macros`, `i-slint-core`, `i-slint-compiler`, `i-slint-backend-winit`, `i-slint-backend-selector` or the two renderers, nor their own missing dependencies (`femtovg`, `accesskit_winit`, `accesskit_unix`, `accesskit_atspi_common`, `atspi`, `icu_locale_fallback`, `pulp`, `equator`, `wayland-protocols-experimental`). Most of the 46 "other" crates are Slint's too: Slint 1.17.1 asks for older series than sid has (`parley` 0.10, `taffy` 0.10, `fontique` 0.10, `resvg` and `usvg` 0.47, `smithay-client-toolkit` 0.20, `calloop` 0.13, `phf` 0.13, …). These move when Debian packages Slint 1.17.1, which needs them.
- **Ours, directly:**
  - `krilla` 0.8 (PDF export) and what it brings: `pdf-writer`, `subsetter`, `xmp-writer` and `write-fonts` are missing, and krilla holds `skrifa` at 0.42 while sid has 0.44.
  - `zip` 9 (Guitar Pro 7–8): sid has 5.1.
  - `quick-xml` 0.42 (GPIF): sid has 0.38.
  - Already in sid at our version: `object` 0.37.3, `libloading`, `postcard`, `serde`, `rfd`, `png`, `tiny-skia` 0.12.

So `debian/` can't build from Debian's crates alone (it builds from a vendor component for now; below) until Debian packages Slint 1.17.1, krilla and its writers, and the newer `zip` and `quick-xml`. The weekly audit shows the gap closing.

## The packaging: `debian/`

`debian/` builds both packages, `oxiguitar` (the Wayland build) and `oxiguitar-plugins-basic`, the way Debian builds them: `dpkg-buildpackage` from a source package, through dh-cargo's cargo wrapper, with Debian's compiler flags, `-dbgsym` packages and no network. The Debian workflow does that in sid on every change to the packaging or to `Cargo.lock`, runs lintian on the source and the packages (errors and warnings fail it), and installs them into a clean sid: the core alone opens nothing, with the plugins it opens every fixture, and the window starts under Weston.

- **The source** comes from `scripts/debian/source.sh`, as version `0.10.0+ds-1` (`+ds`: repacked). The orig tarball is HEAD's tree without `debian/` and without our copy of Noto Sans (`Files-Excluded` in `debian/copyright`).
- **The compiler is Debian's** (`rustc`, `cargo` in `Build-Depends`), as the archive requires; rustup's toolchains can't be used there, since Debian builds offline from its own packages. Our minimum Rust has come from rustup since 24 Oct 2026 (Mateus) and is 1.99, ahead of sid's 1.97.1 (1.98 is in experimental), so the workflow's build is skipped with a notice until sid has it. Before an upload, sid's rustc must have caught up, which is why CONTRIBUTING asks to raise the minimum only when something needs it. (The NAM plugin's NeuralAmpModeler-rs also needs the x86-64-v3 baseline, above Debian's amd64 baseline, so that plugin wouldn't go to Debian as it is.)
- **Noto Sans comes from `fonts-noto-core`.** `debian/rules` links Debian's `NotoSans-Regular.ttf` where ours was, and the programs embed it when they are built (`Built-Using: fonts-noto`). Bravura stays ours (Mateus, 18 Oct 2026). Debian's Noto Sans is older (2.004 against our 2.015), so text in the Debian build may sit a little differently from our references; the visual-regression tests run upstream, with ours.
- **The crates come from `vendor/` for now**, the `orig-vendor` component tarball: `cargo vendor` of `Cargo.lock`, with no prebuilt binaries (the Windows crates' import libraries, test DLLs, WebAssembly objects, a minified web worker) and without pdf-writer's example ICC profile, which doesn't allow changes. Each crate keeps its package checksum, as Cargo.lock pins it. The NOTICE files of the vendored crates are installed with `oxiguitar`. When Debian has every crate (the audit above), `prepare-debian` links Debian's registry instead (`--link-from-system`), `Build-Depends` names the `librust-*-dev` packages, and the vendor component goes.
- **The contact is a placeholder** (Mateus, 18 Oct 2026), never a personal address. Our own .deb packages name the issue tracker, but Debian's tools refuse anything but an email address in `debian/changelog`, so `debian/` says `OxiGuitar <maintainer@oxiguitar.invalid>` (`.invalid` is reserved, so it can never reach anyone). A Debian upload needs its Debian maintainer's address there. CI allows lintian's tags about the contact, and the first upload's `Closes:` (initial-upload-closes-no-bugs) waits for an ITP bug.
- **Not yet:** the X11 build (`oxiguitar-x11`), deprecated, isn't in `debian/`.
