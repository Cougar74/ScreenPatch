use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, IsWindowVisible, MsgWaitForMultipleObjects, PeekMessageW,
    SetWindowDisplayAffinity, TranslateMessage, MSG, PM_REMOVE, QS_ALLINPUT, WDA_EXCLUDEFROMCAPTURE,
};
use xcap::Monitor;

use crate::capture::service::{frame_from_rgba, CaptureError};
use crate::core::geometry::{physical_to_logical_px, CaptureRect, WorkArea};
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

pub fn list_displays() -> Result<Vec<DisplayInfo>, CaptureError> {
    let monitors = Monitor::all().map_err(|err| {
        CaptureError::new(format!("не удалось перечислить мониторы: {err}"))
    })?;
    let work_rects = work_rects();
    let mut displays = Vec::new();
    for monitor in monitors {
        let id = monitor.name().map_err(|err| {
            CaptureError::new(format!("не удалось прочитать имя монитора: {err}"))
        })?;
        let origin_x = monitor.x().map_err(|err| {
            CaptureError::new(format!("не удалось прочитать положение монитора {id}: {err}"))
        })?;
        let origin_y = monitor.y().map_err(|err| {
            CaptureError::new(format!("не удалось прочитать положение монитора {id}: {err}"))
        })?;
        let physical_width = monitor.width().map_err(|err| {
            CaptureError::new(format!("не удалось прочитать размер монитора {id}: {err}"))
        })?;
        let physical_height = monitor.height().map_err(|err| {
            CaptureError::new(format!("не удалось прочитать размер монитора {id}: {err}"))
        })?;
        let scale = quantize_scale(monitor.scale_factor().map_err(|err| {
            CaptureError::new(format!("не удалось прочитать масштаб монитора {id}: {err}"))
        })?);
        let work = logical_work_area(
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
    monitor_id: &str,
    region: CaptureRect,
) -> Result<ImageFrame, CaptureError> {
    if region.x < 0 || region.y < 0 {
        return Err(CaptureError::new(
            "прямоугольник захвата выходит в отрицательные координаты монитора",
        ));
    }
    if crate::core::geometry::frame_byte_len(region.width, region.height).is_none() {
        return Err(CaptureError::new(
            "размер кадра превышает 64 МиБ или переполняет расчёт буфера",
        ));
    }
    let monitors = Monitor::all().map_err(|err| {
        CaptureError::new(format!("не удалось перечислить мониторы: {err}"))
    })?;
    let monitor = monitors
        .into_iter()
        .find(|monitor| monitor.name().ok().as_deref() == Some(monitor_id))
        .ok_or_else(|| CaptureError::new(format!("монитор {monitor_id} не найден")))?;
    let image = monitor
        .capture_region(region.x as u32, region.y as u32, region.width, region.height)
        .map_err(|err| CaptureError::new(err.to_string()))?;
    frame_from_rgba(image.width(), image.height(), image.into_raw())
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
        CaptureError::new("масштаб монитора не позволяет перевести рабочую область")
    })?;
    let y = physical_to_logical_px(rel_y, scale).ok_or_else(|| {
        CaptureError::new("масштаб монитора не позволяет перевести рабочую область")
    })?;
    let (width, height) = crate::core::geometry::logical_window_size(rel_w, rel_h, scale)
        .ok_or_else(|| {
            CaptureError::new("масштаб монитора не позволяет перевести рабочую область")
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
