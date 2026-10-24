use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows::core::{w, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, SetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, RECT, WIN32_ERROR,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, GetSaveFileNameW, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, IsWindowVisible, MessageBoxW, MsgWaitForMultipleObjects, PeekMessageW,
    SetWindowDisplayAffinity, TranslateMessage, IDYES, MB_ICONQUESTION, MB_YESNO, MSG, PM_REMOVE,
    QS_ALLINPUT, WDA_EXCLUDEFROMCAPTURE,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use xcap::Monitor;

use crate::capture::service::{frame_from_rgba, CaptureError};
use crate::core::geometry::{physical_to_logical_px, CaptureRect, WorkArea};
use crate::core::language::{self, Language};
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
