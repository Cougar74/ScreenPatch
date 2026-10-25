use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows::core::{w, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, SetLastError, COLORREF, ERROR_ALREADY_EXISTS, HWND, LPARAM, POINT,
    RECT, SIZE, WIN32_ERROR,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW,
    EnumDisplayMonitors, GetMonitorInfoW, GetTextExtentPoint32W, SelectObject, SetBkMode,
    SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
    DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_TOP,
    FF_DONTCARE, FW_BOLD, FW_NORMAL, OUT_DEFAULT_PRECIS, HDC, HFONT, HGDIOBJ, HMONITOR, MONITORINFO,
    TRANSPARENT,
};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, GetSaveFileNameW, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetCursorPos, IsWindowVisible, MessageBoxW, MsgWaitForMultipleObjects,
    PeekMessageW, SetWindowDisplayAffinity, TranslateMessage, IDYES, MB_ICONQUESTION, MB_YESNO,
    MSG, PM_REMOVE, QS_ALLINPUT, WDA_EXCLUDEFROMCAPTURE,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use xcap::Monitor;

use crate::capture::service::{frame_from_rgba, CaptureError};
use crate::core::geometry::{
    help_line_top, logical_to_physical_px, physical_to_logical_px, CaptureRect, WorkArea,
    HELP_LINE_HEIGHT, HELP_PAD_X,
};
use crate::core::language::{self, help_key, Language};
use crate::core::refresh::{ImageFrame, Fingerprint};

#[derive(Clone, Debug)]
pub struct DisplayInfo {
    pub id: String,
    pub origin_x: i32,
    pub origin_y: i32,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale: f64,
    pub work: WorkArea,
}

impl DisplayInfo {
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint {
            id: self.id.clone(),
            origin_x: self.origin_x,
            origin_y: self.origin_y,
            physical_width: self.physical_width,
            physical_height: self.physical_height,
            scale: self.scale,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Hwnd(pub isize);

struct WorkRect {
    origin_x: i32,
    origin_y: i32,
    width: u32,
    height: u32,
    work_left: i32,
    work_top: i32,
    work_right: i32,
    work_bottom: i32,
}

pub fn list_displays(language: Language) -> Result<Vec<DisplayInfo>, CaptureError> {
    let monitors = Monitor::all().map_err(|err| {
        CaptureError::new(language::render(
            language,
            language::ENUM_MONITORS_FAILED,
            &[("err", &err.to_string())],
        ))
    })?;
    let work_rects = work_rects();
    let mut displays = Vec::new();
    for monitor in monitors {
        let id = monitor.name().map_err(|err| {
            CaptureError::new(language::render(
                language,
                language::MONITOR_NAME_FAILED,
                &[("err", &err.to_string())],
            ))
        })?;
        let origin_x = monitor.x().map_err(|err| {
            CaptureError::new(language::render(
                language,
                language::MONITOR_ORIGIN_FAILED,
                &[("id", &id), ("err", &err.to_string())],
            ))
        })?;
        let origin_y = monitor.y().map_err(|err| {
            CaptureError::new(language::render(
                language,
                language::MONITOR_ORIGIN_FAILED,
                &[("id", &id), ("err", &err.to_string())],
            ))
        })?;
        let physical_width = monitor.width().map_err(|err| {
            CaptureError::new(language::render(
                language,
                language::MONITOR_SIZE_FAILED,
                &[("id", &id), ("err", &err.to_string())],
            ))
        })?;
        let physical_height = monitor.height().map_err(|err| {
            CaptureError::new(language::render(
                language,
                language::MONITOR_SIZE_FAILED,
                &[("id", &id), ("err", &err.to_string())],
            ))
        })?;
        let scale = quantize_scale(monitor.scale_factor().map_err(|err| {
            CaptureError::new(language::render(
                language,
                language::MONITOR_SCALE_FAILED,
                &[("id", &id), ("err", &err.to_string())],
            ))
        })?);
        let work = logical_work_area(
            language,
            origin_x,
            origin_y,
            physical_width,
            physical_height,
            scale,
            &work_rects,
        )?;
        displays.push(DisplayInfo {
            id,
            origin_x,
            origin_y,
            physical_width,
            physical_height,
            scale,
            work,
        });
    }
    Ok(displays)
}

pub fn capture_monitor_region(
    language: Language,
    monitor_id: &str,
    region: CaptureRect,
) -> Result<ImageFrame, CaptureError> {
    if region.x < 0 || region.y < 0 {
        return Err(CaptureError::new(language::text(language, language::NEGATIVE_CAPTURE)));
    }
    if crate::core::geometry::frame_byte_len(region.width, region.height).is_none() {
        return Err(CaptureError::new(language::text(language, language::FRAME_TOO_BIG)));
    }
    let monitors = Monitor::all().map_err(|err| {
        CaptureError::new(language::render(
            language,
            language::ENUM_MONITORS_FAILED,
            &[("err", &err.to_string())],
        ))
    })?;
    let monitor = monitors
        .into_iter()
        .find(|monitor| monitor.name().ok().as_deref() == Some(monitor_id))
        .ok_or_else(|| {
            CaptureError::new(language::render(
                language,
                language::MONITOR_NOT_FOUND,
                &[("id", monitor_id)],
            ))
        })?;
    let image = monitor
        .capture_region(region.x as u32, region.y as u32, region.width, region.height)
        .map_err(|err| CaptureError::new(err.to_string()))?;
    frame_from_rgba(language, image.width(), image.height(), image.into_raw())
}

pub fn try_exclude_from_capture(hwnd: Hwnd) -> bool {
    let handle = HWND(hwnd.0 as *mut c_void);
    unsafe {
        SetWindowDisplayAffinity(handle, WDA_EXCLUDEFROMCAPTURE).is_ok()
    }
}

pub fn wait_until_hidden(hwnds: &[Hwnd]) -> Result<(), ()> {
    let handles: Vec<HWND> = hwnds
        .iter()
        .map(|hwnd| HWND(hwnd.0 as *mut c_void))
        .collect();
    let started = Instant::now();
    let limit = Duration::from_secs(1);
    loop {
        if handles
            .iter()
            .all(|handle| unsafe { !IsWindowVisible(*handle).as_bool() })
        {
            return Ok(());
        }
        if started.elapsed() >= limit {
            return Err(());
        }
        let remaining = limit
            .saturating_sub(started.elapsed())
            .as_millis()
            .min(u128::from(u32::MAX)) as u32;
        let wake = unsafe { MsgWaitForMultipleObjects(None, false, remaining, QS_ALLINPUT) };
        if wake.0 == 0 {
            pump_messages();
        } else {
            return Err(());
        }
    }
}

pub fn hwnd_from_window(window: &winit::window::Window) -> Option<Hwnd> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(Hwnd(win32.hwnd.get())),
        _ => None,
    }
}

fn pump_messages() {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn quantize_scale(scale: f32) -> f64 {
    (f64::from(scale) * 1000.0).round() / 1000.0
}

fn logical_work_area(
    language: Language,
    origin_x: i32,
    origin_y: i32,
    physical_width: u32,
    physical_height: u32,
    scale: f64,
    work_rects: &[WorkRect],
) -> Result<WorkArea, CaptureError> {
    let matched = work_rects.iter().find(|work| {
        work.origin_x == origin_x
            && work.origin_y == origin_y
            && work.width == physical_width
            && work.height == physical_height
    });
    let (left, top, right, bottom) = if let Some(work) = matched {
        (work.work_left, work.work_top, work.work_right, work.work_bottom)
    } else {
        (
            origin_x,
            origin_y,
            origin_x.saturating_add(physical_width as i32),
            origin_y.saturating_add(physical_height as i32),
        )
    };
    let rel_x = left.saturating_sub(origin_x);
    let rel_y = top.saturating_sub(origin_y);
    let rel_w = right.saturating_sub(left).max(0) as u32;
    let rel_h = bottom.saturating_sub(top).max(0) as u32;
    let x = physical_to_logical_px(rel_x, scale).ok_or_else(|| {
        CaptureError::new(language::text(language, language::WORK_AREA_SCALE))
    })?;
    let y = physical_to_logical_px(rel_y, scale).ok_or_else(|| {
        CaptureError::new(language::text(language, language::WORK_AREA_SCALE))
    })?;
    let (width, height) = crate::core::geometry::logical_window_size(rel_w, rel_h, scale)
        .ok_or_else(|| {
            CaptureError::new(language::text(language, language::WORK_AREA_SCALE))
        })?;
    Ok(WorkArea {
        x,
        y,
        width,
        height,
    })
}

fn work_rects() -> Vec<WorkRect> {
    let mut monitors = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitors),
            LPARAM(&mut monitors as *mut Vec<HMONITOR> as isize),
        );
    }
    monitors
        .into_iter()
        .filter_map(|monitor| {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..MONITORINFO::default()
            };
            let ok = unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() };
            if !ok {
                return None;
            }
            let bounds = info.rcMonitor;
            let work = info.rcWork;
            Some(WorkRect {
                origin_x: bounds.left,
                origin_y: bounds.top,
                width: bounds.right.saturating_sub(bounds.left).max(0) as u32,
                height: bounds.bottom.saturating_sub(bounds.top).max(0) as u32,
                work_left: work.left,
                work_top: work.top,
                work_right: work.right,
                work_bottom: work.bottom,
            })
        })
        .collect()
}

pub fn system_ui_language() -> Language {
    let language = unsafe { GetUserDefaultUILanguage() };
    if u32::from(language) & 0x3ff == 0x19 {
        Language::Ru
    } else {
        Language::En
    }
}

pub fn read_saved_language() -> Result<Option<Language>, String> {
    let Some(root) = std::env::var_os("LOCALAPPDATA") else {
        return Err("LOCALAPPDATA".to_string());
    };
    let path = PathBuf::from(root).join("ScreenPatch").join("language.txt");
    match std::fs::read_to_string(&path) {
        Ok(text) if text.trim().is_empty() => Ok(None),
        Ok(text) => language::parse_saved_language(&text)
            .map(Some)
            .ok_or_else(|| text.trim().to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

pub fn write_saved_language(language: Language) -> Result<(), String> {
    let root = std::env::var_os("LOCALAPPDATA").ok_or_else(|| "LOCALAPPDATA".to_string())?;
    let folder = PathBuf::from(root).join("ScreenPatch");
    std::fs::create_dir_all(&folder).map_err(|err| err.to_string())?;
    let word = match language {
        Language::Ru => "ru\n",
        Language::En => "en\n",
    };
    std::fs::write(folder.join("language.txt"), word).map_err(|err| err.to_string())
}

pub fn acquire_single_instance(language: Language) -> bool {
    static INSTANCE_LOCK: OnceLock<isize> = OnceLock::new();
    unsafe {
        SetLastError(WIN32_ERROR(0));
        match CreateMutexW(None, false, w!("Local\\ScreenPatch")) {
            Ok(handle) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    let _ = CloseHandle(handle);
                    false
                } else {
                    let _ = INSTANCE_LOCK.set(handle.0 as isize);
                    true
                }
            }
            Err(err) => {
                eprintln!(
                    "{}",
                    language::render(
                        language,
                        language::SINGLE_INSTANCE_FAILED,
                        &[("err", &err.to_string())]
                    )
                );
                true
            }
        }
    }
}

pub fn confirm_close_all(parent: Option<Hwnd>, question: &str) -> bool {
    unsafe {
        let owner = parent
            .map(|hwnd| HWND(hwnd.0 as *mut c_void))
            .unwrap_or_default();
        let question: Vec<u16> = question.encode_utf16().chain(std::iter::once(0)).collect();
        MessageBoxW(
            Some(owner),
            PCWSTR(question.as_ptr()),
            w!("ScreenPatch"),
            MB_YESNO | MB_ICONQUESTION,
        ) == IDYES
    }
}

pub fn suggested_png_name() -> String {
    let time = unsafe { GetLocalTime() };
    format!(
        "ScreenPatch_{:04}-{:02}-{:02}_{:02}-{:02}-{:02}.png",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
}

pub fn choose_png_path(
    language: Language,
    parent: Option<Hwnd>,
    suggested: &str,
) -> Result<Option<PathBuf>, String> {
    let mut file: Vec<u16> = suggested.encode_utf16().chain(std::iter::once(0)).collect();
    file.resize(1024, 0);
    let filter: Vec<u16> = "PNG\0*.png\0\0".encode_utf16().collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: parent
            .map(|hwnd| HWND(hwnd.0 as *mut c_void))
            .unwrap_or_default(),
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        lpstrDefExt: w!("png"),
        Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
        ..Default::default()
    };
    let chosen = unsafe { GetSaveFileNameW(&mut dialog) };
    if !chosen.as_bool() {
        let code = unsafe { CommDlgExtendedError() };
        if code.0 == 0 {
            return Ok(None);
        }
        return Err(language::render(
            language,
            language::SAVE_DIALOG_FAILED,
            &[("code", &format!("{code:?}"))],
        ));
    }
    let end = file.iter().position(|unit| *unit == 0).unwrap_or(file.len());
    Ok(Some(PathBuf::from(String::from_utf16_lossy(&file[..end]))))
}

pub fn cursor_screen_point() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some((point.x, point.y))
}

pub fn paint_help(width: u32, height: u32, scale: f64, lines: &[&str; 12]) -> Vec<u8> {
    let pixels = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    let mut rgba = vec![32u8, 32, 32, 255].repeat(pixels / 4);
    if width == 0 || height == 0 {
        return rgba;
    }
    unsafe { draw_help_lines(&mut rgba, width, height, scale, lines) };
    rgba
}

unsafe fn draw_help_lines(rgba: &mut [u8], width: u32, height: u32, scale: f64, lines: &[&str; 12]) {
    let dc = CreateCompatibleDC(None);
    if dc.is_invalid() {
        return;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0);
    if bitmap.is_err() || bits.is_null() {
        let _ = DeleteDC(dc);
        return;
    }
    let bitmap = bitmap.unwrap_or_default();
    let bitmap_object = HGDIOBJ::from(bitmap);
    let previous = SelectObject(dc, bitmap_object);
    let count = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    let source = std::slice::from_raw_parts_mut(bits as *mut u8, count.min(rgba.len()));
    for pixel in source.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[32, 32, 32, 255]);
    }
    let font_height = -logical_to_physical_px(16, scale).unwrap_or(16);
    let font = help_font(font_height, FW_NORMAL.0 as i32);
    let bold = help_font(font_height, FW_BOLD.0 as i32);
    let bold_ready = !bold.is_invalid();
    if !font.is_invalid() {
        let font_object = HGDIOBJ::from(font);
        let bold_object = HGDIOBJ::from(bold);
        let previous_font = SelectObject(dc, font_object);
        let _ = SetBkMode(dc, TRANSPARENT);
        let _ = SetTextColor(dc, COLORREF(0x00FF_FFFF));
        let left = logical_to_physical_px(HELP_PAD_X, scale).unwrap_or(HELP_PAD_X);
        let line_height =
            logical_to_physical_px(HELP_LINE_HEIGHT, scale).unwrap_or(HELP_LINE_HEIGHT);
        let right = width as i32 - left;
        for (index, line) in lines.iter().enumerate() {
            let top = logical_to_physical_px(help_line_top(index), scale)
                .unwrap_or(help_line_top(index));
            let mut bounds = RECT {
                left,
                top,
                right,
                bottom: top.saturating_add(line_height),
            };
            if let Some(key) = help_key(line) {
                if bold_ready {
                    let _ = SelectObject(dc, bold_object);
                    draw_help_text(dc, key, &mut bounds);
                    let measured: Vec<u16> = key.encode_utf16().collect();
                    let mut extent = SIZE { cx: 0, cy: 0 };
                    if GetTextExtentPoint32W(dc, &measured, &mut extent).as_bool() {
                        bounds.left = left.saturating_add(extent.cx);
                    }
                    let _ = SelectObject(dc, font_object);
                    draw_help_text(dc, &line[key.len()..], &mut bounds);
                    continue;
                }
            }
            let _ = SelectObject(dc, font_object);
            draw_help_text(dc, line, &mut bounds);
        }
        let _ = SelectObject(dc, previous_font);
        let _ = DeleteObject(font_object);
        if bold_ready {
            let _ = DeleteObject(bold_object);
        }
    } else if bold_ready {
        let _ = DeleteObject(HGDIOBJ::from(bold));
    }
    for (rgba_pixel, bgra) in rgba.chunks_exact_mut(4).zip(source.chunks_exact(4)) {
        rgba_pixel[0] = bgra[2];
        rgba_pixel[1] = bgra[1];
        rgba_pixel[2] = bgra[0];
        rgba_pixel[3] = 255;
    }
    let _ = SelectObject(dc, previous);
    let _ = DeleteObject(bitmap_object);
    let _ = DeleteDC(dc);
}

unsafe fn help_font(height: i32, weight: i32) -> HFONT {
    CreateFontW(
        height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        u32::from(DEFAULT_PITCH.0) | u32::from(FF_DONTCARE.0),
        w!("Segoe UI"),
    )
}

unsafe fn draw_help_text(dc: HDC, text: &str, bounds: &mut RECT) {
    let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = DrawTextW(
        dc,
        &mut wide,
        bounds,
        DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOPREFIX,
    );
}

unsafe extern "system" fn enum_monitors(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let list = &mut *(data.0 as *mut Vec<HMONITOR>);
    list.push(monitor);
    BOOL(1)
}
