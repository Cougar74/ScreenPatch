#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::{
    acquire_single_instance, capture_monitor_region, choose_png_path, confirm_close_all,
    draw_strip, hwnd_from_window, list_displays, suggested_png_name, try_exclude_from_capture,
    wait_until_hidden, DisplayInfo, Hwnd,
};
