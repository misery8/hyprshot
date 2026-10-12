# Changelog

## [1.0.1] - 2026-10-13

Patch release for the installed GTK4 toolbar rendering regression (#33 / PR #34). The core screenshot workflow is unchanged.

### Fixed

- Restore Arrow, Rectangle, Undo and Palette symbolic icon appearance in GTK4 using supported symbolic foreground/stroke attributes and a mask-free palette path with four visible cutouts.
- Load embedded icons by their GTK IconTheme names so symbolic recoloring and 24px tool / 28px Palette glyph sizing work correctly.
- Restore the accepted rounded grouped toolbar surface, 36px main buttons and stable active/inactive visuals across Adwaita light/dark themes.
- Make display-global toolbar CSS registration idempotent across repeated toolbar constructions, while retaining icon resources for surviving toolbars.

### UI polish

- Compact the 17 palette popup swatches to 26×26 GTK logical pixels and slightly increase the selected-color indicator request to 14×14.

### Verification and compatibility

- Exact-head Rust/GTK CI, negative SVG GResource contract checks, repeated-toolbar lifecycle and interaction checks, Arch candidate build, and targeted installed Hyprland visual acceptance completed before release preparation.
- No changes to native capture, clipboard semantics, annotation algorithms, HiDPI/multi-output editor mapping or the future Screen/Video launcher. Outstanding work remains in separate issues #18, #25, #27–#32.

## [1.0.0] - 2026-10-08

First stable release of the current Hyprshot screenshot and annotation workflow.

### Features

- Native Wayland screenshot capture with `ext-image-copy-capture-v1` and a `wlr-screencopy-v1` compatibility fallback when the required ext globals are unavailable.
- No external `grim` screenshot command or `grim` package dependency.
- Quick selection capture and editor mode, with screenshot delivery through the persistent clipboard helper.
- Arrow, rectangle, blur and single-line text annotations, with per-run text colors and regional Undo.
- Dashed selection border, eight resize handles, move/resize, edge containment and resize inversion.
- Direct Cairo image-surface capture path, format/transform handling, and scaled multi-output composition helpers.

### Packaging

- Set Cargo and Arch package metadata to `1.0.0`, targeting release tag `v1.0.0`.
- Remove obsolete direct `grim` and `gdk-pixbuf2` Arch dependencies.
- Desktop entry launches `hyprshot screen` until the separate GUI launcher feature is implemented.
- The Arch package recipe is intended for building **after** `v1.0.0` is tagged. It must not be published against the older `v0.4.0` source.

### Current scope and known limits

- Invoke screenshots with `hyprshot screen`. The no-argument GUI launcher is planned separately in issue #16.
- Final cross-monitor editor interactions and fractional-scale pointer-to-pixel mapping are tracked in issue #18. The capture backend supports multi-output composition; this does not guarantee full cross-monitor editor UX.
- Hyprland screencopy permission enforcement is controlled by the compositor; see README for configuration.
- Video recording is not part of this release.

Release acceptance: exact-tag Arch package build, package metadata inspection, and installed-package smoke test must be completed before publication.
