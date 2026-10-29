#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
mod port;

#[cfg(windows)]
pub use port::WindowsPlatform;
#[cfg(windows)]
pub use windows::{
    acquire_single_instance, capture_monitor_region, choose_png_path, confirm_close_all,
    cursor_screen_point, hwnd_from_window, list_displays, make_tooltip_click_through,
    measure_tooltip_text, paint_help, paint_tooltip, place_tooltip_window, read_saved_language,
    round_tooltip_window, set_capture_excluded, suggested_png_name, system_ui_language,
    try_exclude_from_capture, wait_until_hidden,
    write_saved_language, DisplayInfo, Hwnd,
};
