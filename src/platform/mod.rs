#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::{
    capture_monitor_region, hwnd_from_window, list_displays, try_exclude_from_capture,
    wait_until_hidden, DisplayInfo, Hwnd,
};
