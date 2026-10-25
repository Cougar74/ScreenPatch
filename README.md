# ScreenPatch

[Русская версия](README.ru.md)

Copyright 2026 Eduard

## What it does

ScreenPatch keeps captures in separate windows above ordinary windows. There is no main window.

## Requirements

Windows, and stable Rust with the MSVC toolchain. This repository does not pin a compiler version.

## Build

```
cargo build --release
```

The build writes `target/release/ScreenPatch.exe`. That path is not stored in git. The binary for this version is [release/ScreenPatch.exe](release/ScreenPatch.exe).

## Run

Start `release/ScreenPatch.exe`, or run `cargo run`. A tray icon appears and no window opens. Starting the program again exits immediately and does not add a second icon.

## Language

The first launch follows the Windows interface language. Russian Windows shows Russian labels. Any other language shows English. «Язык» or Language lists «Русский» and English, and the current one is checked. Choosing the other language switches at once and remembers the choice. Choosing the current language changes nothing. The next launch uses that choice instead of the Windows language.

## Capture

Press Ctrl+Shift+S or choose «Новый снимок». Drag a region. A region smaller than 16×16 does not create a window. Esc closes only the selection.

The new window matches the capture: one screen pixel per frame pixel. The buttons start hidden. Right-click shows them, and a second right-click hides them. The picture keeps its physical size. The buttons are round icons: refresh, before and after, copy, save, and close.

Drag the picture to move the window. Drag the corner to resize it. The 120×80 minimum applies only to that corner. Press `1` to restore the original physical size and shift the window into the work area. Press `R` to capture the same region again. Moving the window onto a monitor with a different scale does not change the picture's physical size.

## Windows

«Показать все» and «Скрыть все» change visibility and leave the pictures unchanged. «Закрыть все снимки» asks for confirmation. «Справка» or Help opens one window that lists the hotkeys and mouse actions. Esc or the close button dismisses it. Choosing Help while that window is still open focuses it and does not move it. A separator sits above «Язык», and «Выход» stays below another separator. «Выход» leaves the tray and ends the process. Closing the last capture does not end the process. Show, hide, and close all leave the Help window as it is.

## Privacy

The capture, the clipboard, and the PNG you choose stay on this computer. ScreenPatch does not make network requests of its own.

## Limitations

See [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md).

## License

Apache License, Version 2.0. See [LICENSE](LICENSE).

Copyright 2026 Eduard
