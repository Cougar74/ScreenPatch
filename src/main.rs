use std::num::NonZeroU32;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorGrabMode, CursorIcon, Window, WindowAttributes, WindowId, WindowLevel};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

use screenpatch::core::geometry::{
    logical_to_physical_px, logical_window_size, place_window, CaptureRect, LogicalRect,
};
use screenpatch::core::refresh::{fingerprints_match, refresh_current, ImageFrame};
use screenpatch::core::selection::{compose_overlay, crop_frame, normalize_drag, physical_cursor_point, selection_accepted};
use screenpatch::platform::{
    capture_monitor_region, hwnd_from_window, list_displays, try_exclude_from_capture,
    wait_until_hidden, DisplayInfo, Hwnd,
};

struct PreparedOverlay {
    display: DisplayInfo,
    frame: ImageFrame,
}

struct OverlayWindow {
    window: Arc<Window>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    #[allow(dead_code)]
    context: softbuffer::Context<Arc<Window>>,
    display: DisplayInfo,
    frame: ImageFrame,
    cursor: Option<(i32, i32)>,
}

struct Drag {
    window_id: WindowId,
    anchor: (i32, i32),
    current: (i32, i32),
}

struct PatchWindow {
    window: Arc<Window>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    #[allow(dead_code)]
    context: softbuffer::Context<Arc<Window>>,
    region: CaptureRect,
    frame: ImageFrame,
    fingerprint_id: String,
    origin_x: i32,
    origin_y: i32,
    physical_width: u32,
    physical_height: u32,
    scale: f64,
    affinity: bool,
    refreshing: bool,
}

struct App {
    prepared: Option<Vec<PreparedOverlay>>,
    overlays: Vec<OverlayWindow>,
    windows: Vec<PatchWindow>,
    drag: Option<Drag>,
    started: bool,
    startup_error: Option<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    enable_per_monitor_dpi();
    let displays = list_displays().map_err(|err| err.message)?;
    let prepared = prepare_overlays(&displays)?;
    let event_loop = EventLoop::new().map_err(|err| format!("не удалось создать цикл событий: {err}"))?;
    let mut app = App {
        prepared: Some(prepared),
        overlays: Vec::new(),
        windows: Vec::new(),
        drag: None,
        started: false,
        startup_error: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|err| format!("цикл событий завершился с ошибкой: {err}"))?;
    if let Some(err) = app.startup_error {
        return Err(err);
    }
    Ok(())
}

fn enable_per_monitor_dpi() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

fn prepare_overlays(displays: &[DisplayInfo]) -> Result<Vec<PreparedOverlay>, String> {
    if displays.is_empty() {
        return Err("мониторы не найдены".to_string());
    }
    let mut prepared = Vec::new();
    for display in displays {
        if !display.scale.is_finite() || display.scale <= 0.0 {
            return Err(format!("масштаб монитора {} некорректен", display.id));
        }
        if display.physical_width == 0 || display.physical_height == 0 {
            return Err(format!("размер монитора {} некорректен", display.id));
        }
        let region = CaptureRect {
            x: 0,
            y: 0,
            width: display.physical_width,
            height: display.physical_height,
        };
        let frame = capture_monitor_region(&display.id, region).map_err(|err| {
            format!("захват монитора {} не удался: {}", display.id, err.message)
        })?;
        prepared.push(PreparedOverlay {
            display: display.clone(),
            frame,
        });
    }
    Ok(prepared)
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.started {
            return;
        }
        self.started = true;
        if let Err(err) = self.open_overlays(event_loop) {
            self.startup_error = Some(err);
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.overlay_index(window_id).is_some() {
            self.overlay_event(event_loop, window_id, event);
            return;
        }
        let Some(index) = self
            .windows
            .iter()
            .position(|patch| patch.window.id() == window_id)
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                self.windows.retain(|patch| patch.window.id() != window_id);
                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed || event.repeat {
                    return;
                }
                match event.logical_key {
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Character(text) if text.eq_ignore_ascii_case("r") => self.refresh(index),
                    _ => {}
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let window = Arc::clone(&self.windows[index].window);
                let _ = window.drag_window();
            }
            WindowEvent::RedrawRequested => self.paint_patch(index),
            _ => {}
        }
    }
}

impl App {
    fn overlay_index(&self, window_id: WindowId) -> Option<usize> {
        self.overlays
            .iter()
            .position(|overlay| overlay.window.id() == window_id)
    }

    fn open_overlays(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let prepared = self
            .prepared
            .take()
            .ok_or_else(|| "слои уже созданы".to_string())?;
        for item in prepared {
            let (logical_w, logical_h) = logical_window_size(
                item.display.physical_width,
                item.display.physical_height,
                item.display.scale,
            )
            .ok_or_else(|| format!("масштаб монитора {} некорректен", item.display.id))?;
            let attributes = WindowAttributes::default()
                .with_title(format!("ScreenPatch {}", item.display.id))
                .with_decorations(false)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_inner_size(LogicalSize::new(logical_w as f64, logical_h as f64))
                .with_position(PhysicalPosition::new(
                    item.display.origin_x,
                    item.display.origin_y,
                ));
            let window = Arc::new(event_loop.create_window(attributes).map_err(|err| {
                format!("не удалось создать слой монитора {}: {err}", item.display.id)
            })?);
            window.set_cursor(CursorIcon::Crosshair);
            let context = softbuffer::Context::new(Arc::clone(&window)).map_err(|err| {
                format!("не удалось создать буфер слоя {}: {err}", item.display.id)
            })?;
            let surface = softbuffer::Surface::new(&context, Arc::clone(&window)).map_err(|err| {
                format!("не удалось создать поверхность слоя {}: {err}", item.display.id)
            })?;
            window.request_redraw();
            self.overlays.push(OverlayWindow {
                window,
                context,
                surface,
                display: item.display,
                frame: item.frame,
                cursor: None,
            });
        }
        Ok(())
    }

    fn overlay_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if self
                    .drag
                    .as_ref()
                    .is_some_and(|drag| drag.window_id == window_id)
                {
                    self.drag = None;
                }
                self.overlays
                    .retain(|overlay| overlay.window.id() != window_id);
                if self.overlays.is_empty() && self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && event.logical_key == Key::Named(NamedKey::Escape)
                {
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.note_cursor(window_id, position);
                if self
                    .drag
                    .as_ref()
                    .is_some_and(|drag| drag.window_id == window_id)
                {
                    if let Some(cursor) = self.overlay_cursor(window_id) {
                        if let Some(drag) = self.drag.as_mut() {
                            drag.current = cursor;
                        }
                        self.request_overlay_redraw(window_id);
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.drag.is_some() {
                    return;
                }
                let Some(cursor) = self.overlay_cursor(window_id) else {
                    return;
                };
                self.drag = Some(Drag {
                    window_id,
                    anchor: cursor,
                    current: cursor,
                });
                if let Some(index) = self.overlay_index(window_id) {
                    let _ = self.overlays[index]
                        .window
                        .set_cursor_grab(CursorGrabMode::Confined);
                }
                self.request_overlay_redraw(window_id);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self
                    .drag
                    .as_ref()
                    .is_some_and(|drag| drag.window_id == window_id)
                {
                    self.finish_drag(event_loop, window_id);
                }
            }
            WindowEvent::RedrawRequested => self.paint_overlay(window_id),
            _ => {}
        }
    }

    fn note_cursor(&mut self, window_id: WindowId, position: PhysicalPosition<f64>) {
        let Some(index) = self.overlay_index(window_id) else {
            return;
        };
        let display = &self.overlays[index].display;
        let point = physical_cursor_point(
            position.x,
            position.y,
            display.physical_width,
            display.physical_height,
        );
        self.overlays[index].cursor = Some(point);
    }

    fn overlay_cursor(&self, window_id: WindowId) -> Option<(i32, i32)> {
        self.overlays
            .iter()
            .find(|overlay| overlay.window.id() == window_id)
            .and_then(|overlay| overlay.cursor)
    }

    fn request_overlay_redraw(&self, window_id: WindowId) {
        if let Some(overlay) = self
            .overlays
            .iter()
            .find(|overlay| overlay.window.id() == window_id)
        {
            overlay.window.request_redraw();
        }
    }

    fn finish_drag(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if let Some(index) = self.overlay_index(window_id) {
            let _ = self.overlays[index]
                .window
                .set_cursor_grab(CursorGrabMode::None);
        }
        let rect = normalize_drag(drag.anchor.0, drag.anchor.1, drag.current.0, drag.current.1);
        if !selection_accepted(rect) {
            self.request_overlay_redraw(window_id);
            return;
        }
        let Some(index) = self.overlay_index(window_id) else {
            return;
        };
        let Some(cropped) = crop_frame(&self.overlays[index].frame, rect) else {
            eprintln!(
                "не удалось вырезать область, монитор id={}, прямоугольник=({},{},{}x{})",
                self.overlays[index].display.id, rect.x, rect.y, rect.width, rect.height
            );
            self.request_overlay_redraw(window_id);
            return;
        };
        let display = self.overlays[index].display.clone();
        if let Err(err) = self.spawn_patch(event_loop, &display, rect, cropped) {
            eprintln!(
                "не удалось создать окно снимка: {err}, монитор id={}, прямоугольник=({},{},{}x{})",
                display.id, rect.x, rect.y, rect.width, rect.height
            );
            self.request_overlay_redraw(window_id);
            return;
        }
        self.overlays.clear();
    }

    fn spawn_patch(
        &mut self,
        event_loop: &ActiveEventLoop,
        display: &DisplayInfo,
        region: CaptureRect,
        frame: ImageFrame,
    ) -> Result<(), String> {
        let (logical_w, logical_h) =
            logical_window_size(frame.width, frame.height, display.scale)
                .ok_or_else(|| format!("масштаб монитора {} некорректен", display.id))?;
        let capture = LogicalRect {
            x: physical_to_logical_i32(region.x, display.scale)?,
            y: physical_to_logical_i32(region.y, display.scale)?,
            width: logical_w,
            height: logical_h,
        };
        let (local_x, local_y) = place_window(capture, logical_w, logical_h, display.work);
        let position_x = display.origin_x
            + logical_to_physical_px(local_x, display.scale)
                .ok_or_else(|| format!("не удалось перевести положение окна монитора {}", display.id))?;
        let position_y = display.origin_y
            + logical_to_physical_px(local_y, display.scale)
                .ok_or_else(|| format!("не удалось перевести положение окна монитора {}", display.id))?;
        let attributes = WindowAttributes::default()
            .with_title(format!("ScreenPatch {}", display.id))
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(logical_w as f64, logical_h as f64))
            .with_position(PhysicalPosition::new(position_x, position_y));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|err| format!("не удалось создать окно: {err}"))?,
        );
        let context = softbuffer::Context::new(Arc::clone(&window))
            .map_err(|err| format!("не удалось создать буфер окна: {err}"))?;
        let surface = softbuffer::Surface::new(&context, Arc::clone(&window))
            .map_err(|err| format!("не удалось создать поверхность окна: {err}"))?;
        let hwnd = hwnd_from_window(&window).ok_or_else(|| "у окна нет дескриптора Win32".to_string())?;
        let affinity = try_exclude_from_capture(hwnd);
        log_monitor(display, region, affinity);
        window.request_redraw();
        self.windows.push(PatchWindow {
            window,
            context,
            surface,
            region,
            frame,
            fingerprint_id: display.id.clone(),
            origin_x: display.origin_x,
            origin_y: display.origin_y,
            physical_width: display.physical_width,
            physical_height: display.physical_height,
            scale: display.scale,
            affinity,
            refreshing: false,
        });
        Ok(())
    }

    fn paint_overlay(&mut self, window_id: WindowId) {
        let selection = self.drag.as_ref().and_then(|drag| {
            if drag.window_id == window_id {
                Some(normalize_drag(
                    drag.anchor.0,
                    drag.anchor.1,
                    drag.current.0,
                    drag.current.1,
                ))
            } else {
                None
            }
        });
        let Some(index) = self.overlay_index(window_id) else {
            return;
        };
        let composed = compose_overlay(&self.overlays[index].frame, selection);
        let window = Arc::clone(&self.overlays[index].window);
        present_frame(&window, &mut self.overlays[index].surface, &composed);
    }

    fn refresh(&mut self, index: usize) {
        if self.windows[index].refreshing {
            eprintln!("обновление уже выполняется");
            return;
        }
        let monitor_id = self.windows[index].fingerprint_id.clone();
        let region = self.windows[index].region;
        let saved = screenpatch::core::refresh::Fingerprint {
            id: monitor_id.clone(),
            origin_x: self.windows[index].origin_x,
            origin_y: self.windows[index].origin_y,
            physical_width: self.windows[index].physical_width,
            physical_height: self.windows[index].physical_height,
            scale: self.windows[index].scale,
        };
        let displays = match list_displays() {
            Ok(displays) => displays,
            Err(err) => {
                log_refresh_cancel(&monitor_id, region, &err.message);
                return;
            }
        };
        let Some(display) = displays.iter().find(|display| display.id == monitor_id) else {
            log_refresh_cancel(&monitor_id, region, "монитор больше не подключён");
            return;
        };
        if !fingerprints_match(&saved, &display.fingerprint()) {
            log_refresh_cancel(&monitor_id, region, "геометрия монитора изменилась");
            return;
        }

        let hide = !self.windows[index].affinity;
        let hwnds: Vec<Hwnd> = self
            .windows
            .iter()
            .filter_map(|patch| hwnd_from_window(&patch.window))
            .collect();
        let restore_windows: Vec<Arc<Window>> = self
            .windows
            .iter()
            .map(|patch| Arc::clone(&patch.window))
            .collect();
        if hide {
            for window in &restore_windows {
                window.set_visible(false);
            }
        }
        self.windows[index].refreshing = true;
        let current = self.windows[index].frame.clone();
        let monitor_for_log = monitor_id.clone();
        let next = refresh_current(
            current.clone(),
            true,
            hide,
            || {
                if hide && wait_until_hidden(&hwnds).is_err() {
                    log_refresh_cancel(
                        &monitor_for_log,
                        region,
                        "не удалось подтвердить скрытие окон",
                    );
                    return Err(());
                }
                match capture_monitor_region(&monitor_for_log, region) {
                    Ok(frame) => Ok(frame),
                    Err(err) => {
                        log_refresh_cancel(&monitor_for_log, region, &err.message);
                        Err(())
                    }
                }
            },
            move || {
                for window in &restore_windows {
                    window.set_visible(true);
                }
            },
        );
        self.windows[index].refreshing = false;
        if next != current {
            self.windows[index].frame = next;
            self.windows[index].window.request_redraw();
        }
    }

    fn paint_patch(&mut self, index: usize) {
        let window = Arc::clone(&self.windows[index].window);
        let frame = self.windows[index].frame.clone();
        present_frame(&window, &mut self.windows[index].surface, &frame);
    }
}

fn present_frame(
    window: &Window,
    surface: &mut softbuffer::Surface<Arc<Window>, Arc<Window>>,
    frame: &ImageFrame,
) {
    let size = window.inner_size();
    let Some(width) = NonZeroU32::new(size.width) else {
        return;
    };
    let Some(height) = NonZeroU32::new(size.height) else {
        return;
    };
    if surface.resize(width, height).is_err() {
        return;
    }
    let mut buffer = match surface.buffer_mut() {
        Ok(buffer) => buffer,
        Err(_) => return,
    };
    buffer.fill(0x0018_1818);
    blit_frame(&mut buffer, size.width as usize, frame);
    let _ = buffer.present();
}

fn physical_to_logical_i32(physical: i32, scale: f64) -> Result<i32, String> {
    screenpatch::core::geometry::physical_to_logical_px(physical, scale)
        .ok_or_else(|| "масштаб монитора некорректен".to_string())
}

fn log_monitor(display: &DisplayInfo, region: CaptureRect, affinity: bool) {
    let exclusion = if affinity {
        "display-affinity"
    } else {
        "скрытие"
    };
    eprintln!(
        "монитор id={} начало=({},{}) размер={}x{} масштаб={} прямоугольник=({},{},{}x{}) исключение={}",
        display.id,
        display.origin_x,
        display.origin_y,
        display.physical_width,
        display.physical_height,
        format_scale(display.scale),
        region.x,
        region.y,
        region.width,
        region.height,
        exclusion
    );
}

fn log_refresh_cancel(monitor_id: &str, region: CaptureRect, reason: &str) {
    eprintln!(
        "обновление отменено: {reason}, монитор id={monitor_id}, прямоугольник=({},{},{}x{})",
        region.x, region.y, region.width, region.height
    );
}

fn format_scale(scale: f64) -> String {
    let rounded = (scale * 1000.0).round() / 1000.0;
    format!("{rounded}")
}

fn blit_frame(buffer: &mut [u32], stride: usize, frame: &ImageFrame) {
    let frame_width = frame.width as usize;
    let frame_height = frame.height as usize;
    let rows = buffer.len() / stride.max(1);
    let copy_width = frame_width.min(stride);
    let copy_height = frame_height.min(rows);
    for y in 0..copy_height {
        for x in 0..copy_width {
            let src = (y * frame_width + x) * 4;
            let red = frame.pixels[src] as u32;
            let green = frame.pixels[src + 1] as u32;
            let blue = frame.pixels[src + 2] as u32;
            buffer[y * stride + x] = (red << 16) | (green << 8) | blue;
        }
    }
}
