# Acceptance

The owner checked the behavior below in the checklists for steps 1–6 and window size. Older notes that no longer match the program are not listed. Esc on the last window does not end the process, and startup shows only the tray.

- [x] Ctrl+Shift+S or «Новый снимок» starts a selection.
- [x] A region of at least 16×16 opens one borderless window at the original pixel size. The button strip is hidden.
- [x] A second capture stays independent of the first.
- [x] Dragging one window does not move another. The corner resizes that window, and the picture keeps its proportions.
- [x] `R` captures the original region again and does not include a ScreenPatch window.
- [x] After a refresh, «До/после» shows the previous picture, and a second press returns the new one.
- [x] «Копировать» places the shown picture on the clipboard. «Сохранить» writes a PNG at the original resolution.
- [x] «Скрыть все» hides the windows. «Показать все» brings them back with the same pictures.
- [x] Refusing «Закрыть все снимки» leaves the windows. Accepting it closes them and leaves the tray icon.
- [x] Unplugging the source monitor leaves the window where Windows put it. `R` does not change the picture, and the tray icon remains.
- [x] Changing the source monitor's scale or resolution makes `R` keep the old picture. Restoring the old geometry lets `R` capture that region again.
- [x] If one monitor cannot be captured for a new selection, the selection overlays do not remain, and open captures keep their pictures and visibility.
- [x] Moving a capture from a 100% monitor to a 125% monitor keeps the picture's physical pixel size.
- [x] `1` restores the original pixel size and shifts the window into the work area when it does not fit.
- [x] The first right-click shows the strip without changing the picture's physical size. The second right-click, with no corner drag between the clicks, hides the strip and restores the previous height.
- [x] «Выход» removes the tray icon and ends the process. Closing the last window does not.
