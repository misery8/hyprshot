# Hyprshot

> A fast, annotation-enabled screenshot tool for **Hyprland** (Wayland), built with **Rust** and **GTK 4**.
> Capture, annotate, and copy — all in seconds.

![Hyprshot Editor Preview](preview.png)

- **Instant launcher** via `PrintScreen`
- **Draw shapes** and **blur** directly on your screenshot
- **Copies result to clipboard** — no file clutter
- **Two modes**:
    - **Quick capture**: select -> release -> done
    - **Editor mode**: `Ctrl` + select -> annotate -> `Ctrl+S` to copy

Perfect for quick sharing, bug reporting, or visual notes — without leaving your keyboard.

---

## Usage

### Quick Capture
1. Press `PrintScreen`
2. Drag to select an area
3. Release mouse -> image is copied to clipboard

### Editor Mode
1. Press `PrintScreen`
2. Drag to select an area
3. **Press `Ctrl`** -> editor panel appears
4. Draw shapes, blur sensitive data, adjust selection
5. Press `Ctrl+S` -> annotated image is copied to clipboard

> No UI windows, no dialogs — just pure speed.

---

## Installation

### From Source

Make sure you have:
- Rust (1.75+)
- `gtk4`, `glib2`, `cairo` development headers

```sh
git clone [https://github.com/misery8/hyprshot.git](https://github.com/misery8/hyprshot.git)
cd hyprshot
cargo build --release
sudo install -Dm755 target/release/hyprshot /usr/local/bin/hyprshot
```

### Arch Linux (AUR-coming soon)

```sh
yay -S hyprshot-git
```

## Dependencies

- Runtime:
    - `gtk4`, `glib2`, `cairo`
    - `grim`, `slurp` (essential for screen capture)

___

## Configuration (Hyprland)

Add to your `~/.config/hypr/hyprland.conf`:

```ini
bindl = ,Print, exec, hyprshot screen
```

## License
GPL-3.0-or-later - free and open for all.

> Made with ❤️ for the Hyprland community.