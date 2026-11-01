# Changelog

## 0.4.0

The Windows binary is `release/ScreenPatch.exe`.

- Each capture has an opacity slider. The picture fades; the buttons, the slider, and the lock stay solid. Copy and a saved PNG use the original opaque pixels.
- A lock at the top-right of the control strip pins position and size. The other controls still work, and the lock stays in effect while the strip is hidden.
- The six shortcuts can be changed from the tray. The defaults remain `Ctrl+Shift+S`, `Ctrl+Alt+Shift+S`, `R`, `1`, `S`, and `Esc`. Help shows the combinations that are saved.
- A tray checkbox, Copy new captures, puts each new capture on the clipboard and still opens its window. Refresh does not copy. The choice is remembered.
- The tray lists New capture, Show all, Hide all, and Close all captures, then Copy new captures, Shortcuts, and Language.

## 0.3.0

The Windows binary is `release/ScreenPatch.exe`.

- Hovering a capture button shows its name. The tip follows the cursor.
- `Ctrl+Alt+Shift+S` captures a region and keeps visible ScreenPatch windows in the picture: other captures, Help, and a tooltip that is showing. Hidden captures stay out. `Ctrl+Shift+S`, the tray item, and `S` still leave the app's capture windows out.
- Refreshing a capture made with `Ctrl+Alt+Shift+S` keeps those windows in the new picture. The window being refreshed is not part of that picture. Before/after still returns the previous frame.

## 0.2.0

The Windows binary is `release/ScreenPatch.exe`.

- Capture buttons are round icons: refresh, before/after, copy, save, and close. Holding a button darkens it until release.
- Right-click shows or hides those buttons. Dragging the picture still moves the window.
- The tray language entry is a submenu, «Язык» or Language. It lists «Русский» and English, and the current one is checked.
- «Справка» or Help opens one window with the hotkeys and mouse actions. The key combinations are bold. Esc or the close button dismisses it, and choosing Help again focuses the same window.

## 0.1.1

The Windows binary is `release/ScreenPatch.exe`.

- Explorer shows the application icon, and the file properties name the program ScreenPatch.
- The release build does not open a console window.

## 0.1.0

First release. The Windows binary is `release/screenpatch.exe`.

- The program starts in the tray, and a second start exits.
- A new capture comes from Ctrl+Shift+S or «Новый снимок».
- Each capture opens in its own window at the original pixel size. The button strip starts hidden.
- The strip can refresh, compare before and after, copy, save a PNG, and close.
- The tray can show, hide, or close every capture. «Выход» ends the process.
- The interface and log follow Russian or English. The tray remembers the choice.
