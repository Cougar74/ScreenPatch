<div align="center">

# 📌 ScreenPatch

### Keep what matters in sight.

**Capture any part of your screen and keep it floating above your workspace.**  
A lightweight, local-first Windows utility built with Rust.

[![Version](https://img.shields.io/badge/version-0.2.0-2563eb?style=flat-square)](https://github.com/Cougar74/ScreenPatch/blob/main/CHANGELOG.md)
![Platform](https://img.shields.io/badge/platform-Windows-0078D4?style=flat-square&logo=windows)
![Rust](https://img.shields.io/badge/built_with-Rust-CE412B?style=flat-square&logo=rust&logoColor=white)
[![License](https://img.shields.io/badge/license-Apache--2.0-green?style=flat-square)](LICENSE)

[**Download for Windows**](https://github.com/Cougar74/ScreenPatch/blob/main/release/ScreenPatch.exe) · [Features](#-features) · [Getting started](#-getting-started) · [Roadmap](#-roadmap) · [🇷🇺 Русский](README.ru.md)

</div>

---

> **A reference without the window switching.** Pin a code snippet, design, chart, or any screen detail while you keep working elsewhere.

<!-- Add a real screenshot or demo GIF before enabling this image.
![ScreenPatch in action](assets/screenpatch-demo.gif)
-->

## ✨ Features

| | |
|---|---|
| 📌 **Floating captures** | Every capture lives in its own movable, resizable window above regular windows. No main window required. |
| ✂️ **Region selection** | Press `Ctrl + Shift + S` and drag to capture an area. |
| 🔄 **Refresh in place** | Press `R` to capture the original region again. |
| 🔀 **Before / after** | Switch between the current and previous capture after refreshing. |
| 📋 **Copy or save** | Copy the displayed image to the clipboard or save a full-resolution PNG. |
| 🖥️ **Multi-monitor aware** | Keeps the image's physical pixel size when moving across displays with different scaling. |
| 🌍 **English & Russian** | Automatic initial language selection, with a remembered manual override. |
| 🔒 **Local-first** | No network requests initiated by ScreenPatch; captures stay on your computer. |

## 🚀 Getting started

**Windows:** [Download `ScreenPatch.exe`](https://github.com/Cougar74/ScreenPatch/blob/main/release/ScreenPatch.exe) from the repository and run it. ScreenPatch starts in the system tray—there is no main application window.

1. Press **`Ctrl + Shift + S`** (or select **New capture** in the tray).
2. **Drag** over the area you want to keep visible.
3. **Move or resize** the floating image and continue your work.
4. **Right-click** the image to reveal **Refresh · Before/after · Copy · Save · Close**.

**Tip:** `R` refreshes the original region, `1` restores its original physical size, and `Esc` cancels region selection.

## 💡 Made for your workflow

- **Developers:** keep a specification, error message, or code example in view.
- **Designers:** compare a reference with the UI you are building.
- **QA engineers:** refresh a captured interface and switch between before and after.
- **Everyone:** pin instructions, a chart, or a small piece of information without juggling windows.

## ⌨️ Controls

| Action | Control |
|---|---|
| New capture | `Ctrl + Shift + S` or tray menu |
| Select region | Left-click and drag |
| Cancel selection | `Esc` |
| Move capture | Drag the image |
| Resize capture | Drag its corner |
| Show / hide controls | Right-click the image |
| Refresh original region | `R` while the capture is active |
| Restore original size | `1` while the capture is active |
| Copy / Save / Close | Buttons revealed by right-click |
| Show / hide / close all | Tray menu |

Select **Help** from the tray for the in-app reference. The app remains in the tray when the last capture is closed; use **Exit** to quit.

## 🛠️ Build from source

Requires **Windows**, stable **Rust** and the **MSVC toolchain**.

```powershell
git clone https://github.com/Cougar74/ScreenPatch.git
cd ScreenPatch
cargo build --release
```

The output is `target/release/ScreenPatch.exe`. During development, use `cargo run`.

## 🧭 Roadmap

Ideas being considered (not available in v0.2.0):

- [ ] **Customization:** opacity, lock position, configurable global hotkey
- [ ] **Live references:** timed refresh, optional visual change highlights
- [ ] **Workspace:** session restore, capture groups, layouts
- [ ] **Better capture:** multi-monitor resilience and improved region selection
- [ ] **Sharing:** quick annotations and privacy-aware redaction

Have an idea? [Open an issue](https://github.com/Cougar74/ScreenPatch/issues). This list is exploratory, not a release commitment.

## ⚠️ Current limitations

ScreenPatch v0.2.0 runs only on Windows and supports up to **10** open captures. It has a **512 MiB** combined frame budget and a **64 MiB** maximum per frame. It does not yet support automatic refresh, restored sessions, or captures anchored to another application's window. Changes in source-monitor geometry can prevent refresh until the original geometry returns.

See [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md) for the full list.

## 🤝 Contributing

Bug reports, usability suggestions and pull requests are welcome. Before larger changes, please [open an issue](https://github.com/Cougar74/ScreenPatch/issues) to discuss the approach. Review [ACCEPTANCE.md](ACCEPTANCE.md) for existing behavioral expectations and [CHANGELOG.md](CHANGELOG.md) for release history.

## 📄 License

Licensed under [Apache License 2.0](LICENSE). Copyright © 2026 Eduard.

---

<div align="center"><sub>Built with 🦀 Rust. Keep your reference close, and your workspace clear.</sub></div>
