# 🌊 Aqua (RustyOS)

> A sleek, elegant **macOS Sequoia-styled Wayland compositor** written in 100% Rust.
> Rendered with CPU precision via `tiny-skia` and powered by `smithay`.

---

## ✨ Features

- **macOS Sequoia Visual Standard**: Pixel-perfect transparent top panel, glass Control Center, Spotlight search (`⌘ + Space`), and floating Dock.
- **Pure Rust Stack**: Zero C, C++, or heavy GTK/Qt dependencies.
- **Native Hardware (DRM/KMS)**: Runs directly on your GPU from a clean TTY or display manager (`sddm`, `gdm`, `ly`) via `libseat` and `libinput`.
- **Nested Testing Mode**: Can also run in a window inside existing Wayland/X11 desktops for rapid development.
- **Smart Window Management**: Native support for CSD (Cursor, VS Code, Chrome) and SSD (traffic lights, animations, smooth resizing, maximizing).

---

## 🚀 Easy Installation (Arch Linux)

### 1. One-line installation:
```bash
git clone https://github.com/ragezorrr/RustyOS.git
cd RustyOS
chmod +x install.sh
./install.sh
```

Or directly via curl:
```bash
curl -sSL https://raw.githubusercontent.com/ragezorrr/RustyOS/main/install.sh | bash
```

The script will automatically:
1. Install all dependencies (`seatd`, `libinput`, `mesa`, `rust`).
2. Build the optimized release binary.
3. Install `/usr/local/bin/aqua`.
4. Register the session in `/usr/share/wayland-sessions/aqua.desktop`.

---

## 🔄 Updating Aqua

Whenever a new version is published to GitHub, update anytime with a single command:

```bash
aqua-update
```

It will pull the latest commits, rebuild the binary, and update your system installation.

---

## 📦 Arch Linux PKGBUILD

You can also build and install Aqua as a native Arch package:
```bash
makepkg -si
```

---

## 🎮 Launching Aqua

- **From Display Manager**: Select **Aqua (macOS Sequoia)** at your login screen.
- **From clean TTY**: Just run:
  ```bash
  aqua
  ```
- **Nested in an existing desktop**:
  ```bash
  aqua
  ```

---

## 📜 License
MIT License.
