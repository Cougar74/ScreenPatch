# Known limitations

ScreenPatch 0.4.0 has these limits:

- Windows only.
- At most 10 capture windows.
- The 512 MiB budget counts the current frame and the previous frame.
- One frame cannot be larger than 64 MiB.
- No timed refresh, no saved sessions, and no capture pinned to another application's window.
- Unplugging the source monitor does not close the window. `R` keeps the old picture and logs the reason.
- If the source monitor's geometry changes, `R` also keeps the old picture until that geometry returns.
- If capture fails for any monitor, the whole new selection is cancelled.
- Shortcuts can be changed from the tray. The names below are the defaults.
- If the new-capture shortcut cannot be registered, a new capture is still available from the tray. The log still names `Ctrl+Shift+S`.
- If the inclusive shortcut cannot be registered, that gesture is unavailable. The other new-capture shortcut and the tray still start a capture without the app windows. The log still names `Ctrl+Alt+Shift+S`.
- This repository has no screenshots.
