#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::{
    acquire_single_instance, capture_monitor_region, choose_png_path, confirm_close_all,
    draw_strip, hwnd_from_window, list_displays, read_saved_language, suggested_png_name,
    system_ui_language, try_exclude_from_capture, wait_until_hidden, write_saved_language,
    DisplayInfo, Hwnd,
};
