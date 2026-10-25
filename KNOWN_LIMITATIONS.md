# Known limitations

ScreenPatch 0.2.0 has these limits:

- Windows only.
- At most 10 capture windows.
- The 512 MiB budget counts the current frame and the previous frame.
- One frame cannot be larger than 64 MiB.
- No timed refresh, no saved sessions, and no capture pinned to another application's window.
- Unplugging the source monitor does not close the window. `R` keeps the old picture and logs the reason.
- If the source monitor's geometry changes, `R` also keeps the old picture until that geometry returns.
- If capture fails for any monitor, the whole new selection is cancelled.
- If Ctrl+Shift+S is already taken, a new capture is still available from the tray.
- This repository has no screenshots.
