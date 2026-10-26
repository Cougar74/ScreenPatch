use std::path::Path;
use std::time::SystemTime;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOW};

use crate::core::geometry::CaptureRect;
use crate::core::language::{self, Language};
use crate::core::port::PlatformPort;
use crate::core::refresh::Fingerprint;
use super::windows::{
    capture_monitor_region, list_displays, read_saved_language, system_ui_language, wait_until_hidden,
    write_saved_language, Hwnd,
};

pub struct WindowsPlatform {
    language: Language,
}

impl WindowsPlatform {
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    fn with_hidden<T>(&self, hide: &[Hwnd], body: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        for hwnd in hide {
            unsafe {
                let _ = ShowWindow(HWND(hwnd.0 as *mut std::ffi::c_void), SW_HIDE);
            }
        }
        let restore = ShowAgain(hide);
        let result = if hide.is_empty() || wait_until_hidden(hide).is_ok() {
            body()
        } else {
            Err(language::text(self.language, language::HIDE_FAILED).to_string())
        };
        drop(restore);
        result
    }
}

struct ShowAgain<'a>(&'a [Hwnd]);

impl Drop for ShowAgain<'_> {
    fn drop(&mut self) {
        for hwnd in self.0 {
            unsafe {
                let _ = ShowWindow(HWND(hwnd.0 as *mut std::ffi::c_void), SW_SHOW);
            }
        }
    }
}

impl PlatformPort for WindowsPlatform {
    type Window = Hwnd;

    fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    fn system_language(&self) -> Language {
        system_ui_language()
    }

    fn read_language(&self) -> Result<Option<Language>, String> {
        read_saved_language()
    }

    fn write_language(&mut self, language: Language) -> Result<(), String> {
        write_saved_language(language)
    }

    fn monitors(&self) -> Result<Vec<Fingerprint>, String> {
        list_displays(Language::En)
            .map(|displays| displays.into_iter().map(|display| display.fingerprint()).collect())
            .map_err(|err| err.message)
    }

    fn snapshot(&mut self, hide: &[Hwnd]) -> Result<Vec<(Fingerprint, Vec<u8>)>, String> {
        self.with_hidden(hide, || {
            let displays = list_displays(self.language).map_err(|err| err.message)?;
            let mut frames = Vec::new();
            for display in displays {
                let region = CaptureRect {
                    x: 0,
                    y: 0,
                    width: display.physical_width,
                    height: display.physical_height,
                };
                match capture_monitor_region(self.language, &display.id, region) {
                    Ok(frame) => frames.push((display.fingerprint(), frame.pixels.to_vec())),
                    Err(err) => {
                        return Err(language::render(
                            self.language,
                            language::CAPTURE_MONITOR_FAILED,
                            &[("id", &display.id), ("err", &err.message)],
                        ));
                    }
                }
            }
            Ok(frames)
        })
    }

    fn capture_region(
        &mut self,
        monitor_id: &str,
        rect: &CaptureRect,
        hide: &[Hwnd],
    ) -> Result<(u32, u32, Vec<u8>), String> {
        self.with_hidden(hide, || {
            let frame = capture_monitor_region(self.language, monitor_id, *rect).map_err(|err| err.message)?;
            Ok((frame.width, frame.height, frame.pixels.to_vec()))
        })
    }

    fn write_png(&mut self, path: &Path, pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
        image::save_buffer(path, pixels, width, height, image::ExtendedColorType::Rgba8)
            .map_err(|err| err.to_string())
    }

    fn copy_image(&mut self, pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
        let image = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Borrowed(pixels),
        };
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_image(image))
            .map_err(|err| err.to_string())
    }
}
