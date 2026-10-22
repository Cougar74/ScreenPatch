use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorGrabMode, CursorIcon, Window, WindowAttributes, WindowId, WindowLevel};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};

use screenpatch::core::commands::{client_hit, PanelAction};
use screenpatch::core::geometry::{
    logical_client_keeping_physical, logical_to_physical_px, logical_window_size,
    one_to_one_physical_size, place_window, shift_into_work_area, CaptureRect, LogicalRect,
    WorkArea,
};
use screenpatch::core::patches::{
    letterbox, AddPatchError, CommitRefreshError, PatchManager,
};
use screenpatch::core::refresh::{refresh_current, refresh_gate, ImageFrame, RefreshBlock};
use screenpatch::core::selection::{
    compose_overlay, crop_frame, normalize_drag, physical_cursor_point, selection_accepted,
    selection_ready, MonitorReadiness, SelectionAbort,
};
use screenpatch::platform::{
    acquire_single_instance, capture_monitor_region, choose_png_path, confirm_close_all,
    draw_strip, hwnd_from_window, list_displays, suggested_png_name, try_exclude_from_capture,
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

struct CornerResize {
    window_id: WindowId,
    anchor: (i32, i32),
    width: u32,
    height: u32,
}

struct PatchWindow {
    number: u64,
    window: Arc<Window>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    #[allow(dead_code)]
    context: softbuffer::Context<Arc<Window>>,
    region: CaptureRect,
    frame: ImageFrame,
    cursor: Option<(i32, i32)>,
    fingerprint_id: String,
    origin_x: i32,
    origin_y: i32,
    physical_width: u32,
    physical_height: u32,
    scale: f64,
    affinity: bool,
    refreshing: bool,
    strip_visible: bool,
    physical_client: (u32, u32),
}

struct App {
    prepared: Option<Vec<PreparedOverlay>>,
    overlays: Vec<OverlayWindow>,
    windows: Vec<PatchWindow>,
    patches: PatchManager,
    drag: Option<Drag>,
    corner: Option<CornerResize>,
    started: bool,
    startup_error: Option<String>,
    tray: Option<TrayIcon>,
    menu_items: Vec<MenuItem>,
    hotkeys: Option<GlobalHotKeyManager>,
    hotkey_id: u32,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    if !acquire_single_instance() {
        eprintln!("ScreenPatch уже запущен");
        return Ok(());
    }
    enable_per_monitor_dpi();
    let event_loop = EventLoop::new().map_err(|err| format!("не удалось создать цикл событий: {err}"))?;
    let mut app = App {
        prepared: None,
        overlays: Vec::new(),
        windows: Vec::new(),
        patches: PatchManager::new(),
        drag: None,
        corner: None,
        started: false,
        startup_error: None,
        tray: None,
        menu_items: Vec::new(),
        hotkeys: None,
        hotkey_id: 0,
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
    let mut readiness = Vec::new();
    let mut prepared = Vec::new();
    let mut capture_error = None;
    for display in displays {
        let scale_ok = display.scale.is_finite() && display.scale > 0.0;
        let size_ok = display.physical_width > 0 && display.physical_height > 0;
        let mut frame_ready = false;
        if scale_ok && size_ok {
            let region = CaptureRect {
                x: 0,
                y: 0,
                width: display.physical_width,
                height: display.physical_height,
            };
            match capture_monitor_region(&display.id, region) {
                Ok(frame) => {
                    frame_ready = true;
                    prepared.push(PreparedOverlay {
                        display: display.clone(),
                        frame,
                    });
                }
                Err(err) => {
                    if capture_error.is_none() {
                        capture_error = Some(err.message);
                    }
                }
            }
        }
        readiness.push(MonitorReadiness {
            id: display.id.clone(),
            scale: display.scale,
            physical_width: display.physical_width,
            physical_height: display.physical_height,
            frame_ready,
        });
    }
    match selection_ready(&readiness) {
        Ok(()) => Ok(prepared),
        Err(SelectionAbort::NoMonitors) => Err("мониторы не найдены".to_string()),
        Err(SelectionAbort::BadScale { id }) => Err(format!("масштаб монитора {id} некорректен")),
        Err(SelectionAbort::EmptySize { id }) => Err(format!("размер монитора {id} некорректен")),
        Err(SelectionAbort::CaptureFailed { id }) => Err(format!(
            "не удалось снять монитор {id}: {}",
            capture_error.unwrap_or_default()
        )),
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.started {
            return;
        }
        self.started = true;
        event_loop.set_control_flow(ControlFlow::Wait);
        if let Err(err) = self.install_shell() {
            self.startup_error = Some(err);
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.poll_shell(event_loop);
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
            WindowEvent::CloseRequested => self.close_patch(index),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                // При закрытии окна фокус уходит на следующее, и winit заново
                // сообщает уже зажатые клавиши. Такое сообщение не новое нажатие.
                if event.state != ElementState::Pressed || event.repeat || is_synthetic {
                    return;
                }
                match event.logical_key {
                    Key::Named(NamedKey::Escape) => self.close_patch(index),
                    Key::Character(text) if text.eq_ignore_ascii_case("r") => self.refresh(index),
                    Key::Character(text) if text == "1" => self.restore_actual_size(index),
                    Key::Character(text) if text.eq_ignore_ascii_case("s") => {
                        self.begin_selection(event_loop)
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.windows[index].window.scale_factor();
                self.windows[index].cursor = logical_client_point(position.x, position.y, scale);
                self.update_patch_cursor(index);
                self.apply_corner_resize(index);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.begin_patch_pointer(index),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => self.toggle_strip(index),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.end_corner_resize(index),
            WindowEvent::Resized(_) => {
                self.remember_physical_client(index);
                self.windows[index].window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let (width, height) = self.windows[index].physical_client;
                match logical_client_keeping_physical(width, height, scale_factor) {
                    Some(_) => {
                        let _ = self.windows[index]
                            .window
                            .request_inner_size(PhysicalSize::new(width, height));
                    }
                    None => eprintln!(
                        "масштаб окна некорректен, размер снимка {} не изменён",
                        self.windows[index].number
                    ),
                }
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
            let attributes = apply_window_icon(
                WindowAttributes::default()
                    .with_title(format!("ScreenPatch {}", item.display.id))
                    .with_decorations(false)
                    .with_resizable(false)
                    .with_window_level(WindowLevel::AlwaysOnTop)
                    .with_inner_size(LogicalSize::new(logical_w as f64, logical_h as f64))
                    .with_position(PhysicalPosition::new(
                        item.display.origin_x,
                        item.display.origin_y,
                    )),
            );
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
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && !is_synthetic
                    && event.logical_key == Key::Named(NamedKey::Escape)
                {
                    self.dismiss_overlays(event_loop);
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
        self.patches
            .check(&frame)
            .map_err(|err| patch_add_error(&frame, err))?;
        let (region_w, region_h) =
            logical_window_size(frame.width, frame.height, display.scale)
                .ok_or_else(|| format!("масштаб монитора {} некорректен", display.id))?;
        let (physical_w, physical_h) =
            one_to_one_physical_size(frame.width, frame.height, display.scale, false)
                .ok_or_else(|| format!("масштаб монитора {} некорректен", display.id))?;
        let (logical_w, logical_h) =
            logical_client_keeping_physical(physical_w, physical_h, display.scale)
                .ok_or_else(|| format!("масштаб монитора {} некорректен", display.id))?;
        let capture = LogicalRect {
            x: physical_to_logical_i32(region.x, display.scale)?,
            y: physical_to_logical_i32(region.y, display.scale)?,
            width: region_w,
            height: region_h,
        };
        let (local_x, local_y) = place_window(capture, logical_w, logical_h, display.work);
        let position_x = display.origin_x
            + logical_to_physical_px(local_x, display.scale)
                .ok_or_else(|| format!("не удалось перевести положение окна монитора {}", display.id))?;
        let position_y = display.origin_y
            + logical_to_physical_px(local_y, display.scale)
                .ok_or_else(|| format!("не удалось перевести положение окна монитора {}", display.id))?;
        let attributes = apply_window_icon(
            WindowAttributes::default()
                .with_title("ScreenPatch")
                .with_decorations(false)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_inner_size(PhysicalSize::new(physical_w, physical_h))
                .with_position(PhysicalPosition::new(position_x, position_y)),
        );
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
        let number = match self.patches.add(region, frame.clone()) {
            Ok(number) => number,
            Err(err) => return Err(patch_add_error(&frame, err)),
        };
        window.set_title(&format!("ScreenPatch {number}"));
        let _ = window.request_inner_size(PhysicalSize::new(physical_w, physical_h));
        log_monitor(display, region, affinity);
        window.request_redraw();
        self.windows.push(PatchWindow {
            number,
            window,
            context,
            surface,
            region,
            frame,
            cursor: None,
            fingerprint_id: display.id.clone(),
            origin_x: display.origin_x,
            origin_y: display.origin_y,
            physical_width: display.physical_width,
            physical_height: display.physical_height,
            scale: display.scale,
            affinity,
            refreshing: false,
            strip_visible: false,
            physical_client: (physical_w, physical_h),
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
        let monitors: Vec<screenpatch::core::refresh::Fingerprint> = displays
            .iter()
            .map(|display| display.fingerprint())
            .collect();
        match refresh_gate(&saved, &monitors) {
            Ok(()) => {}
            Err(RefreshBlock::MonitorMissing) => {
                log_refresh_cancel(&monitor_id, region, "монитор больше не подключён");
                return;
            }
            Err(RefreshBlock::GeometryChanged) => {
                log_refresh_cancel(&monitor_id, region, "геометрия монитора изменилась");
                return;
            }
        }

        let hide = !self.windows[index].affinity;
        let hwnds: Vec<Hwnd> = self
            .windows
            .iter()
            .filter_map(|patch| hwnd_from_window(&patch.window))
            .collect();
        let restore_windows: Vec<(Arc<Window>, bool)> = self
            .windows
            .iter()
            .map(|patch| {
                (
                    Arc::clone(&patch.window),
                    patch.window.is_visible().unwrap_or(true),
                )
            })
            .collect();
        if hide {
            for (window, _) in &restore_windows {
                window.set_visible(false);
            }
        }
        self.windows[index].refreshing = true;
        let current = self.windows[index].frame.clone();
        let monitor_for_log = monitor_id.clone();
        let mut captured = None;
        let _ = refresh_current(
            current,
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
                    Ok(frame) => {
                        captured = Some(frame.clone());
                        Ok(frame)
                    }
                    Err(err) => {
                        log_refresh_cancel(&monitor_for_log, region, &err.message);
                        Err(())
                    }
                }
            },
            move || {
                for (window, visible) in &restore_windows {
                    window.set_visible(*visible);
                }
            },
        );
        self.windows[index].refreshing = false;
        let Some(frame) = captured else {
            return;
        };
        let number = self.windows[index].number;
        match self.patches.commit_refresh(number, frame.clone()) {
            Ok(()) => {
                self.windows[index].frame = frame;
                self.windows[index].window.request_redraw();
            }
            Err(CommitRefreshError::OverBudget {
                frame_bytes,
                used_bytes,
            }) => {
                eprintln!(
                    "не удалось обновить снимок {number}: кадр {}x{} занимает {frame_bytes} байт, уже занято {used_bytes} байт, монитор id={monitor_id}",
                    frame.width, frame.height
                );
            }
            Err(CommitRefreshError::SizeMismatch) => {
                eprintln!(
                    "не удалось обновить снимок {number}: размер кадра {}x{} не совпал с прямоугольником {}x{}, монитор id={monitor_id}",
                    frame.width, frame.height, region.width, region.height
                );
            }
        }
    }

    fn paint_patch(&mut self, index: usize) {
        let window = Arc::clone(&self.windows[index].window);
        let size = window.inner_size();
        let number = self.windows[index].number;
        let shown = self
            .patches
            .shown_frame(number)
            .cloned()
            .unwrap_or_else(|| self.windows[index].frame.clone());
        let before_enabled = self
            .patches
            .get(number)
            .is_some_and(|patch| patch.previous.is_some());
        let scale = window.scale_factor();
        let strip = if self.windows[index].strip_visible {
            logical_to_physical_px(48, scale).unwrap_or(0).max(0) as u32
        } else {
            0
        };
        let strip = strip.min(size.height);
        let labels = if strip == 0 {
            Vec::new()
        } else {
            draw_strip(size.width, strip, before_enabled)
        };
        let frame = compose_patch(&shown, size.width, size.height, strip, &labels);
        present_frame(&window, &mut self.windows[index].surface, &frame);
        self.remember_physical_client(index);
    }

    fn remember_physical_client(&mut self, index: usize) {
        let size = self.windows[index].window.inner_size();
        if size.width > 0 && size.height > 0 {
            self.windows[index].physical_client = (size.width, size.height);
        }
    }

    fn restore_actual_size(&mut self, index: usize) {
        let number = self.windows[index].number;
        let frame = self
            .patches
            .shown_frame(number)
            .cloned()
            .unwrap_or_else(|| self.windows[index].frame.clone());
        let scale = self.windows[index].window.scale_factor();
        let Some((width, height)) =
            one_to_one_physical_size(frame.width, frame.height, scale, self.windows[index].strip_visible)
        else {
            eprintln!("масштаб окна некорректен, размер снимка {number} не изменён");
            return;
        };
        let window = Arc::clone(&self.windows[index].window);
        let _ = window.request_inner_size(PhysicalSize::new(width, height));
        self.windows[index].physical_client = (width, height);
        let Ok(position) = window.outer_position() else {
            eprintln!("не удалось прочитать положение снимка {number}, окно оставлено на месте");
            return;
        };
        let displays = match list_displays() {
            Ok(displays) => displays,
            Err(err) => {
                eprintln!(
                    "не удалось прочитать рабочую область снимка {number}: {}, окно оставлено на месте",
                    err.message
                );
                return;
            }
        };
        let Some(work) = work_area_for_window(position.x, position.y, width, height, &displays) else {
            eprintln!(
                "не удалось прочитать рабочую область снимка {number}, окно оставлено на месте"
            );
            return;
        };
        let (x, y) = shift_into_work_area(position.x, position.y, width, height, work);
        window.set_outer_position(PhysicalPosition::new(x, y));
    }

    fn toggle_strip(&mut self, index: usize) {
        let number = self.windows[index].number;
        let scale = self.windows[index].window.scale_factor();
        let Some(strip) = logical_to_physical_px(48, scale).filter(|strip| *strip > 0) else {
            eprintln!("масштаб окна некорректен, размер снимка {number} не изменён");
            return;
        };
        let strip = strip as u32;
        let (width, height) = self.windows[index].physical_client;
        let height = if self.windows[index].strip_visible {
            height.saturating_sub(strip).max(1)
        } else {
            height.saturating_add(strip)
        };
        self.windows[index].strip_visible = !self.windows[index].strip_visible;
        let window = Arc::clone(&self.windows[index].window);
        let _ = window.request_inner_size(PhysicalSize::new(width, height));
        self.windows[index].physical_client = (width, height);
        window.request_redraw();
    }

    fn dismiss_overlays(&mut self, _event_loop: &ActiveEventLoop) {
        self.drag = None;
        self.overlays.clear();
    }

    fn close_patch(&mut self, index: usize) {
        let number = self.windows[index].number;
        let window_id = self.windows[index].window.id();
        if self
            .corner
            .as_ref()
            .is_some_and(|corner| corner.window_id == window_id)
        {
            self.corner = None;
        }
        self.patches.close(number);
        self.windows.remove(index);
    }

    fn begin_selection(&mut self, event_loop: &ActiveEventLoop) {
        if !self.overlays.is_empty() {
            return;
        }
        if self.patches.len() >= 10 {
            eprintln!("лимит окон достигнут: уже открыто 10 снимков");
            return;
        }
        let remembered: Vec<(Arc<Window>, bool)> = self
            .windows
            .iter()
            .map(|patch| {
                (
                    Arc::clone(&patch.window),
                    patch.window.is_visible().unwrap_or(true),
                )
            })
            .collect();
        match self.capture_prepared() {
            Ok(prepared) => {
                self.prepared = Some(prepared);
                if let Err(err) = self.open_overlays(event_loop) {
                    eprintln!("не удалось открыть слои выделения: {err}");
                    self.abort_selection(&remembered);
                }
            }
            Err(err) => {
                eprintln!("не удалось начать новое выделение: {err}");
                self.abort_selection(&remembered);
            }
        }
    }

    fn abort_selection(&mut self, remembered: &[(Arc<Window>, bool)]) {
        self.drag = None;
        self.prepared = None;
        self.overlays.clear();
        for (window, visible) in remembered {
            window.set_visible(*visible);
        }
    }

    fn capture_prepared(&mut self) -> Result<Vec<PreparedOverlay>, String> {
        let hidden: Vec<(Arc<Window>, bool)> = self
            .windows
            .iter()
            .filter(|patch| !patch.affinity)
            .map(|patch| {
                (
                    Arc::clone(&patch.window),
                    patch.window.is_visible().unwrap_or(true),
                )
            })
            .collect();
        let hwnds: Vec<Hwnd> = hidden
            .iter()
            .filter_map(|(window, _)| hwnd_from_window(window))
            .collect();
        for (window, _) in &hidden {
            window.set_visible(false);
        }
        let result = (|| {
            if !hidden.is_empty() && wait_until_hidden(&hwnds).is_err() {
                return Err("не удалось подтвердить скрытие окон".to_string());
            }
            let displays = list_displays().map_err(|err| format!("мониторы не найдены: {}", err.message))?;
            prepare_overlays(&displays)
        })();
        for (window, visible) in &hidden {
            window.set_visible(*visible);
        }
        result
    }

    fn begin_patch_pointer(&mut self, index: usize) {
        let window = Arc::clone(&self.windows[index].window);
        let (width, height) = logical_client_size(&window);
        let strip_visible = self.windows[index].strip_visible;
        let Some((x, y)) = self.windows[index].cursor else {
            return;
        };
        match client_hit(x, y, width, height, strip_visible) {
            PanelAction::Resize => {
                // Курсор не ограничиваем окном: иначе его нельзя вывести за край
                // и окно получается только уменьшить.
                self.corner = Some(CornerResize {
                    window_id: window.id(),
                    anchor: (x, y),
                    width,
                    height,
                });
            }
            PanelAction::Drag => {
                let _ = window.drag_window();
            }
            PanelAction::Refresh => self.refresh(index),
            PanelAction::BeforeAfter => {
                let number = self.windows[index].number;
                let _ = self.patches.toggle_shown(number);
                window.request_redraw();
            }
            PanelAction::Copy => self.copy_shown(index),
            PanelAction::Save => self.save_shown(index),
            PanelAction::Close => self.close_patch(index),
        }
    }

    fn update_patch_cursor(&self, index: usize) {
        let window = &self.windows[index].window;
        let resizing = self
            .corner
            .as_ref()
            .is_some_and(|corner| corner.window_id == window.id());
        let (width, height) = logical_client_size(window);
        let strip_visible = self.windows[index].strip_visible;
        let icon = if resizing {
            CursorIcon::NwseResize
        } else {
            match self.windows[index]
                .cursor
                .map(|(x, y)| client_hit(x, y, width, height, strip_visible))
            {
                Some(PanelAction::Resize) => CursorIcon::NwseResize,
                _ => CursorIcon::Default,
            }
        };
        window.set_cursor(icon);
    }

    fn apply_corner_resize(&mut self, index: usize) {
        let window_id = self.windows[index].window.id();
        let Some(corner) = self.corner.as_ref() else {
            return;
        };
        if corner.window_id != window_id {
            return;
        }
        let Some(cursor) = self.windows[index].cursor else {
            return;
        };
        let width = (corner.width as i32 + cursor.0 - corner.anchor.0).max(120) as u32;
        let height = (corner.height as i32 + cursor.1 - corner.anchor.1).max(80) as u32;
        let window = Arc::clone(&self.windows[index].window);
        let _ = window.request_inner_size(LogicalSize::new(f64::from(width), f64::from(height)));
    }

    fn end_corner_resize(&mut self, index: usize) {
        let window_id = self.windows[index].window.id();
        if self
            .corner
            .as_ref()
            .is_some_and(|corner| corner.window_id == window_id)
        {
            self.corner = None;
            self.update_patch_cursor(index);
        }
    }

    fn install_shell(&mut self) -> Result<(), String> {
        let menu = Menu::new();
        let items = vec![
            MenuItem::with_id("new", "Новый снимок", true, None),
            MenuItem::with_id("show", "Показать все", true, None),
            MenuItem::with_id("hide", "Скрыть все", true, None),
            MenuItem::with_id("close-all", "Закрыть все снимки", true, None),
            MenuItem::with_id("exit", "Выход", true, None),
        ];
        let refs: Vec<&dyn tray_icon::menu::IsMenuItem> =
            items.iter().map(|item| item as _).collect();
        menu.append_items(&refs)
            .map_err(|err| format!("не удалось собрать меню значка: {err}"))?;
        let source = tray_icon_source()?;
        let icon = Icon::from_rgba(source.pixels, source.width, source.height)
            .map_err(|err| format!("не удалось создать значок: {err}"))?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ScreenPatch")
            .with_icon(icon)
            .build()
            .map_err(|err| format!("не удалось создать значок в области уведомлений: {err}"))?;
        self.menu_items = items;
        self.tray = Some(tray);
        match register_hotkey() {
            Ok((manager, id)) => {
                self.hotkeys = Some(manager);
                self.hotkey_id = id;
            }
            Err(err) => eprintln!(
                "сочетание Ctrl+Shift+S занято, новый снимок доступен из значка: {err}"
            ),
        }
        Ok(())
    }

    fn poll_shell(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            self.menu_command(event_loop, &event.id);
        }
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if self.hotkeys.is_some()
                && event.state == HotKeyState::Pressed
                && event.id == self.hotkey_id
            {
                self.begin_selection(event_loop);
            }
        }
    }

    fn menu_command(&mut self, event_loop: &ActiveEventLoop, id: &MenuId) {
        if id == "new" {
            self.begin_selection(event_loop);
        } else if id == "show" {
            self.set_patch_visibility(true);
        } else if id == "hide" {
            self.set_patch_visibility(false);
        } else if id == "close-all" {
            self.close_all_patches();
        } else if id == "exit" {
            self.exit_app(event_loop);
        }
    }

    fn set_patch_visibility(&mut self, visible: bool) {
        self.patches.set_windows_visible(visible);
        for patch in &self.windows {
            patch.window.set_visible(visible);
        }
    }

    fn close_all_patches(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        let parent = hwnd_from_window(&self.windows[0].window);
        if !confirm_close_all(parent) {
            return;
        }
        self.corner = None;
        self.windows.clear();
        self.patches.close_all(true);
    }

    fn exit_app(&mut self, event_loop: &ActiveEventLoop) {
        self.drag = None;
        self.corner = None;
        self.overlays.clear();
        self.windows.clear();
        self.patches.close_all(true);
        self.tray = None;
        event_loop.exit();
    }

    fn copy_shown(&self, index: usize) {
        let number = self.windows[index].number;
        let Some(frame) = self.patches.shown_frame(number) else {
            return;
        };
        let image = arboard::ImageData {
            width: frame.width as usize,
            height: frame.height as usize,
            bytes: std::borrow::Cow::Borrowed(frame.pixels.as_ref()),
        };
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_image(image)) {
            Ok(()) => {}
            Err(err) => eprintln!("не удалось скопировать снимок {number}: {err}"),
        }
    }

    fn save_shown(&self, index: usize) {
        let number = self.windows[index].number;
        let Some(frame) = self.patches.shown_frame(number).cloned() else {
            return;
        };
        let parent = hwnd_from_window(&self.windows[index].window);
        let path = match choose_png_path(parent, &suggested_png_name()) {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(err) => {
                eprintln!("не удалось сохранить снимок {number}: {err}");
                return;
            }
        };
        if let Err(err) = image::save_buffer(
            &path,
            frame.pixels.as_ref(),
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgba8,
        ) {
            eprintln!("не удалось сохранить снимок {number}: {err}");
        }
    }
}

struct AppIcon {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

fn app_icon() -> Result<&'static AppIcon, String> {
    static ICON: OnceLock<Result<AppIcon, String>> = OnceLock::new();
    ICON.get_or_init(|| {
        let image = image::load_from_memory(include_bytes!("icon.png"))
            .map_err(|err| format!("не удалось прочитать значок: {err}"))?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(AppIcon {
            pixels: rgba.into_raw(),
            width,
            height,
        })
    })
    .as_ref()
    .map_err(Clone::clone)
}

fn tray_icon_source() -> Result<AppIcon, String> {
    let source = app_icon()?;
    let image = image::RgbaImage::from_raw(source.width, source.height, source.pixels.clone())
        .ok_or_else(|| "не удалось прочитать значок".to_string())?;
    let (width, height) = image.dimensions();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] == 0 {
            continue;
        }
        found = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let cropped = if found {
        image::imageops::crop_imm(&image, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
            .to_image()
    } else {
        image
    };
    let crop_w = cropped.width();
    let crop_h = cropped.height();
    let side = crop_w.max(crop_h).max(1);
    let mut square = image::RgbaImage::from_pixel(side, side, image::Rgba([0, 0, 0, 0]));
    image::imageops::overlay(
        &mut square,
        &cropped,
        i64::from((side - crop_w) / 2),
        i64::from((side - crop_h) / 2),
    );
    let target = tray_slot_px();
    let resized = image::imageops::resize(
        &square,
        target,
        target,
        image::imageops::FilterType::Lanczos3,
    );
    let (width, height) = resized.dimensions();
    Ok(AppIcon {
        pixels: resized.into_raw(),
        width,
        height,
    })
}

fn tray_slot_px() -> u32 {
    let side = unsafe { GetSystemMetrics(SM_CXSMICON) };
    u32::try_from(side).unwrap_or(32).max(32)
}

fn apply_window_icon(attributes: WindowAttributes) -> WindowAttributes {
    let source = match app_icon() {
        Ok(source) => source,
        Err(err) => {
            eprintln!("{err}");
            return attributes;
        }
    };
    match winit::window::Icon::from_rgba(source.pixels.clone(), source.width, source.height) {
        Ok(icon) => attributes.with_window_icon(Some(icon)),
        Err(err) => {
            eprintln!("не удалось поставить значок окна: {err}");
            attributes
        }
    }
}

fn register_hotkey() -> Result<(GlobalHotKeyManager, u32), String> {
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyS);
    let id = hotkey.id();
    let manager = GlobalHotKeyManager::new().map_err(|err| err.to_string())?;
    manager.register(hotkey).map_err(|err| err.to_string())?;
    Ok((manager, id))
}

fn compose_patch(
    frame: &ImageFrame,
    width: u32,
    height: u32,
    strip: u32,
    labels: &[u8],
) -> ImageFrame {
    let width = width.max(1);
    let height = height.max(1);
    let mut pixels = vec![24u8, 24, 24, 255].repeat(width as usize * height as usize);
    let image_height = height.saturating_sub(strip);
    if let Some(placed) = letterbox(width, image_height, frame.width, frame.height) {
        for local_y in 0..placed.height {
            let source_y =
                ((u64::from(local_y) * u64::from(frame.height)) / u64::from(placed.height)) as u32;
            for local_x in 0..placed.width {
                let source_x =
                    ((u64::from(local_x) * u64::from(frame.width)) / u64::from(placed.width)) as u32;
                let source = ((source_y * frame.width + source_x) * 4) as usize;
                let destination_y = strip + placed.y + local_y;
                let destination =
                    ((destination_y * width + placed.x + local_x) * 4) as usize;
                pixels[destination..destination + 4]
                    .copy_from_slice(&frame.pixels[source..source + 4]);
            }
        }
    }
    let strip_bytes = (width as usize)
        .saturating_mul(strip.min(height) as usize)
        .saturating_mul(4)
        .min(labels.len())
        .min(pixels.len());
    pixels[..strip_bytes].copy_from_slice(&labels[..strip_bytes]);
    ImageFrame {
        pixels: Arc::from(pixels.into_boxed_slice()),
        width,
        height,
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

fn patch_add_error(frame: &ImageFrame, err: AddPatchError) -> String {
    match err {
        AddPatchError::TooMany => "лимит окон достигнут".to_string(),
        AddPatchError::OverBudget {
            frame_bytes,
            used_bytes,
        } => format!(
            "кадр {}x{} занимает {frame_bytes} байт, уже занято {used_bytes} байт",
            frame.width, frame.height
        ),
    }
}

fn work_area_for_window(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    displays: &[DisplayInfo],
) -> Option<WorkArea> {
    let center_x = x.saturating_add(i32::try_from(width / 2).unwrap_or(i32::MAX));
    let center_y = y.saturating_add(i32::try_from(height / 2).unwrap_or(i32::MAX));
    if let Some(display) = displays.iter().find(|display| {
        center_x >= display.origin_x
            && center_y >= display.origin_y
            && center_x < display.origin_x.saturating_add(display.physical_width as i32)
            && center_y < display.origin_y.saturating_add(display.physical_height as i32)
    }) {
        return Some(display.work);
    }
    let mut best: Option<(i64, WorkArea)> = None;
    for display in displays {
        let area = overlap_area(display, x, y, width, height);
        if area > 0 && best.as_ref().is_none_or(|(best_area, _)| area > *best_area) {
            best = Some((area, display.work));
        }
    }
    best.map(|(_, work)| work)
}

fn overlap_area(display: &DisplayInfo, x: i32, y: i32, width: u32, height: u32) -> i64 {
    let left = x.max(display.origin_x);
    let top = y.max(display.origin_y);
    let right = x
        .saturating_add(width as i32)
        .min(display.origin_x.saturating_add(display.physical_width as i32));
    let bottom = y
        .saturating_add(height as i32)
        .min(display.origin_y.saturating_add(display.physical_height as i32));
    let overlap_w = i64::from(right.saturating_sub(left)).max(0);
    let overlap_h = i64::from(bottom.saturating_sub(top)).max(0);
    overlap_w.saturating_mul(overlap_h)
}

fn logical_client_point(x: f64, y: f64, scale: f64) -> Option<(i32, i32)> {
    if !x.is_finite() || !y.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    Some((round_half_away(x / scale), round_half_away(y / scale)))
}

fn logical_client_size(window: &Window) -> (u32, u32) {
    let scale = window.scale_factor();
    if !scale.is_finite() || scale <= 0.0 {
        return (120, 80);
    }
    let size = window.inner_size().to_logical::<f64>(scale);
    (
        u32::try_from(round_half_away(size.width).max(0)).unwrap_or(120),
        u32::try_from(round_half_away(size.height).max(0)).unwrap_or(80),
    )
}

fn round_half_away(value: f64) -> i32 {
    let rounded = value.abs().round();
    if value < 0.0 {
        -(rounded as i32)
    } else {
        rounded as i32
    }
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
