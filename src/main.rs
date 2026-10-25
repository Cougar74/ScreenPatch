#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
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

use screenpatch::core::commands::{
    client_hit, decode_button_icon, draw_buttons, strip_press_target, ButtonIcon, PanelAction,
};
use screenpatch::core::language::{
    self, help_lines, language_choice, Language, LANGUAGE_NAME_EN, LANGUAGE_NAME_RU,
};
use screenpatch::core::geometry::{
    logical_client_keeping_physical, logical_to_physical_px, logical_window_size,
    one_to_one_physical_size, place_window, shift_into_work_area, CaptureRect, LogicalRect,
    WorkArea, HELP_CLIENT_HEIGHT, HELP_CLIENT_WIDTH, help_window_origin, index_containing_point,
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
    cursor_screen_point, hwnd_from_window, list_displays, paint_help, read_saved_language,
    suggested_png_name, system_ui_language, try_exclude_from_capture, wait_until_hidden,
    write_saved_language, DisplayInfo, Hwnd,
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
    strip_press: Option<PanelAction>,
    physical_client: (u32, u32),
}

struct TrayMenus {
    // Эти поля не читаются: они держат пункты живыми, пока меню висит в трее.
    #[allow(dead_code)]
    items: Vec<MenuItem>,
    #[allow(dead_code)]
    language_menu: Submenu,
    russian: CheckMenuItem,
    english: CheckMenuItem,
}

struct HelpWindow {
    window: Arc<Window>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    #[allow(dead_code)]
    context: softbuffer::Context<Arc<Window>>,
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
    tray_menus: Option<TrayMenus>,
    help: Option<HelpWindow>,
    hotkeys: Option<GlobalHotKeyManager>,
    hotkey_id: u32,
    language: Language,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let language = match read_saved_language() {
        Ok(Some(language)) => language,
        Ok(None) => system_ui_language(),
        Err(err) => {
            let fallback = system_ui_language();
            eprintln!(
                "{}",
                language::render(fallback, language::LANGUAGE_READ_FAILED, &[("err", &err)])
            );
            fallback
        }
    };
    if !acquire_single_instance(language) {
        eprintln!("{}", language::text(language, language::ALREADY_RUNNING));
        return Ok(());
    }
    let _ = button_icons(language);
    enable_per_monitor_dpi();
    let event_loop = EventLoop::new().map_err(|err| {
        language::render(language, language::EVENT_LOOP_CREATE_FAILED, &[("err", &err.to_string())])
    })?;
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
        tray_menus: None,
        help: None,
        hotkeys: None,
        hotkey_id: 0,
        language,
    };
    event_loop.run_app(&mut app).map_err(|err| {
        language::render(
            language,
            language::EVENT_LOOP_RUN_FAILED,
            &[("err", &err.to_string())],
        )
    })?;
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

fn prepare_overlays(
    language: Language,
    displays: &[DisplayInfo],
) -> Result<Vec<PreparedOverlay>, String> {
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
            match capture_monitor_region(language, &display.id, region) {
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
        Err(SelectionAbort::NoMonitors) => Err(language::text(language, language::NO_MONITORS).to_string()),
        Err(SelectionAbort::BadScale { id }) => Err(language::render(
            language,
            language::BAD_MONITOR_SCALE,
            &[("id", &id)],
        )),
        Err(SelectionAbort::EmptySize { id }) => Err(language::render(
            language,
            language::BAD_MONITOR_SIZE,
            &[("id", &id)],
        )),
        Err(SelectionAbort::CaptureFailed { id }) => Err(language::render(
            language,
            language::CAPTURE_MONITOR_FAILED,
            &[("id", &id), ("err", &capture_error.unwrap_or_default())],
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
        if self
            .help
            .as_ref()
            .is_some_and(|help| help.window.id() == window_id)
        {
            self.help_event(event);
            return;
        }
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
                if self.windows[index].strip_press.is_some() {
                    self.windows[index].window.request_redraw();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.windows[index].cursor = None;
                if self.windows[index].strip_press.is_some() {
                    self.windows[index].window.request_redraw();
                }
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
            } => {
                if !self.finish_strip_press(index) {
                    self.end_corner_resize(index);
                }
            }
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
                        "{}",
                        self.fill(language::BAD_WINDOW_SCALE, &[("number", &self.windows[index].number.to_string())])
                    ),
                }
            }
            WindowEvent::RedrawRequested => self.paint_patch(index),
            _ => {}
        }
    }
}

impl App {
    fn say(&self, phrase: language::Phrase) -> &'static str {
        language::text(self.language, phrase)
    }

    fn fill(&self, phrase: language::Phrase, pairs: &[(&str, &str)]) -> String {
        language::render(self.language, phrase, pairs)
    }

    fn overlay_index(&self, window_id: WindowId) -> Option<usize> {
        self.overlays
            .iter()
            .position(|overlay| overlay.window.id() == window_id)
    }

    fn open_overlays(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let prepared = self
            .prepared
            .take()
            .ok_or_else(|| self.say(language::LAYERS_ALREADY).to_string())?;
        for item in prepared {
            let (logical_w, logical_h) = logical_window_size(
                item.display.physical_width,
                item.display.physical_height,
                item.display.scale,
            )
            .ok_or_else(|| self.fill(language::BAD_MONITOR_SCALE, &[("id", &item.display.id)]))?;
            let attributes = apply_window_icon(self.language,
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
                self.fill(language::OVERLAY_CREATE_FAILED, &[("id", &item.display.id), ("err", &err.to_string())])
            })?);
            window.set_cursor(CursorIcon::Crosshair);
            let context = softbuffer::Context::new(Arc::clone(&window)).map_err(|err| {
                self.fill(language::OVERLAY_BUFFER_FAILED, &[("id", &item.display.id), ("err", &err.to_string())])
            })?;
            let surface = softbuffer::Surface::new(&context, Arc::clone(&window)).map_err(|err| {
                self.fill(language::OVERLAY_SURFACE_FAILED, &[("id", &item.display.id), ("err", &err.to_string())])
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
                "{}",
                self.fill(
                    language::CROP_FAILED,
                    &[
                        ("id", &self.overlays[index].display.id),
                        ("x", &rect.x.to_string()),
                        ("y", &rect.y.to_string()),
                        ("w", &rect.width.to_string()),
                        ("h", &rect.height.to_string()),
                    ],
                )
            );
            self.request_overlay_redraw(window_id);
            return;
        };
        let display = self.overlays[index].display.clone();
        if let Err(err) = self.spawn_patch(event_loop, &display, rect, cropped) {
            eprintln!(
                "{}",
                self.fill(
                    language::PATCH_WINDOW_FAILED,
                    &[
                        ("err", &err),
                        ("id", &display.id),
                        ("x", &rect.x.to_string()),
                        ("y", &rect.y.to_string()),
                        ("w", &rect.width.to_string()),
                        ("h", &rect.height.to_string()),
                    ],
                )
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
            .map_err(|err| patch_add_error(self.language, &frame, err))?;
        let (region_w, region_h) =
            logical_window_size(frame.width, frame.height, display.scale)
                .ok_or_else(|| self.fill(language::BAD_MONITOR_SCALE, &[("id", &display.id)]))?;
        let (physical_w, physical_h) =
            one_to_one_physical_size(frame.width, frame.height, display.scale)
                .ok_or_else(|| self.fill(language::BAD_MONITOR_SCALE, &[("id", &display.id)]))?;
        let (logical_w, logical_h) =
            logical_client_keeping_physical(physical_w, physical_h, display.scale)
                .ok_or_else(|| self.fill(language::BAD_MONITOR_SCALE, &[("id", &display.id)]))?;
        let capture = LogicalRect {
            x: physical_to_logical_i32(self.language, region.x, display.scale)?,
            y: physical_to_logical_i32(self.language, region.y, display.scale)?,
            width: region_w,
            height: region_h,
        };
        let (local_x, local_y) = place_window(capture, logical_w, logical_h, display.work);
        let position_x = display.origin_x
            + logical_to_physical_px(local_x, display.scale)
                .ok_or_else(|| self.fill(language::WINDOW_POS_FAILED, &[("id", &display.id)]))?;
        let position_y = display.origin_y
            + logical_to_physical_px(local_y, display.scale)
                .ok_or_else(|| self.fill(language::WINDOW_POS_FAILED, &[("id", &display.id)]))?;
        let attributes = apply_window_icon(
            self.language,
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
                .map_err(|err| self.fill(language::WINDOW_CREATE_FAILED, &[("err", &err.to_string())]))?,
        );
        let context = softbuffer::Context::new(Arc::clone(&window))
            .map_err(|err| self.fill(language::WINDOW_BUFFER_FAILED, &[("err", &err.to_string())]))?;
        let surface = softbuffer::Surface::new(&context, Arc::clone(&window))
            .map_err(|err| self.fill(language::WINDOW_SURFACE_FAILED, &[("err", &err.to_string())]))?;
        let hwnd = hwnd_from_window(&window).ok_or_else(|| self.say(language::NO_HWND).to_string())?;
        let affinity = try_exclude_from_capture(hwnd);
        let number = match self.patches.add(region, frame.clone()) {
            Ok(number) => number,
            Err(err) => return Err(patch_add_error(self.language, &frame, err)),
        };
        window.set_title(&format!("ScreenPatch {number}"));
        let _ = window.request_inner_size(PhysicalSize::new(physical_w, physical_h));
        log_monitor(self.language, display, region, affinity);
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
            strip_press: None,
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
            eprintln!("{}", self.say(language::REFRESH_IN_PROGRESS));
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
        let displays = match list_displays(self.language) {
            Ok(displays) => displays,
            Err(err) => {
                log_refresh_cancel(self.language, &monitor_id, region, &err.message);
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
                log_refresh_cancel(self.language, &monitor_id, region, self.say(language::MONITOR_MISSING));
                return;
            }
            Err(RefreshBlock::GeometryChanged) => {
                log_refresh_cancel(self.language, &monitor_id, region, self.say(language::GEOMETRY_CHANGED));
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
                        self.language,
                        &monitor_for_log,
                        region,
                        self.say(language::HIDE_FAILED),
                    );
                    return Err(());
                }
                match capture_monitor_region(self.language, &monitor_for_log, region) {
                    Ok(frame) => {
                        captured = Some(frame.clone());
                        Ok(frame)
                    }
                    Err(err) => {
                        log_refresh_cancel(self.language, &monitor_for_log, region, &err.message);
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
                    "{}",
                    self.fill(
                        language::REFRESH_BUDGET,
                        &[
                            ("number", &number.to_string()),
                            ("w", &frame.width.to_string()),
                            ("h", &frame.height.to_string()),
                            ("bytes", &frame_bytes.to_string()),
                            ("used", &used_bytes.to_string()),
                            ("id", &monitor_id),
                        ],
                    )
                );
            }
            Err(CommitRefreshError::SizeMismatch) => {
                eprintln!(
                    "{}",
                    self.fill(
                        language::REFRESH_SIZE,
                        &[
                            ("number", &number.to_string()),
                            ("w", &frame.width.to_string()),
                            ("h", &frame.height.to_string()),
                            ("rw", &region.width.to_string()),
                            ("rh", &region.height.to_string()),
                            ("id", &monitor_id),
                        ],
                    )
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
        let scale = window.scale_factor();
        let mut frame = compose_patch(&shown, size.width, size.height);
        if self.windows[index].strip_visible {
            let mut pixels = frame.pixels.to_vec();
            draw_buttons(
                &mut pixels,
                frame.width,
                frame.height,
                scale,
                self.strip_press_visual(index),
                button_icons(self.language),
            );
            frame.pixels = Arc::from(pixels.into_boxed_slice());
        }
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
            one_to_one_physical_size(frame.width, frame.height, scale)
        else {
            eprintln!("{}", self.fill(language::BAD_WINDOW_SCALE, &[("number", &number.to_string())]));
            return;
        };
        let window = Arc::clone(&self.windows[index].window);
        let _ = window.request_inner_size(PhysicalSize::new(width, height));
        self.windows[index].physical_client = (width, height);
        let Ok(position) = window.outer_position() else {
            eprintln!("{}", self.fill(language::POSITION_UNREAD, &[("number", &number.to_string())]));
            return;
        };
        let displays = match list_displays(self.language) {
            Ok(displays) => displays,
            Err(err) => {
                eprintln!(
                    "{}",
                    self.fill(
                        language::WORK_AREA_UNREAD_ERR,
                        &[("number", &number.to_string()), ("err", &err.message)],
                    )
                );
                return;
            }
        };
        let Some(work) = work_area_for_window(position.x, position.y, width, height, &displays) else {
            eprintln!(
                "{}",
                self.fill(language::WORK_AREA_UNREAD, &[("number", &number.to_string())])
            );
            return;
        };
        let (x, y) = shift_into_work_area(position.x, position.y, width, height, work);
        window.set_outer_position(PhysicalPosition::new(x, y));
    }

    fn toggle_strip(&mut self, index: usize) {
        self.windows[index].strip_press = None;
        self.windows[index].strip_visible = !self.windows[index].strip_visible;
        self.windows[index].window.request_redraw();
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
            eprintln!("{}", self.say(language::WINDOW_LIMIT));
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
                    eprintln!("{}", self.fill(language::LAYERS_FAILED, &[("err", &err)]));
                    self.abort_selection(&remembered);
                }
            }
            Err(err) => {
                eprintln!("{}", self.fill(language::SELECTION_FAILED, &[("err", &err)]));
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
                return Err(self.say(language::HIDE_FAILED).to_string());
            }
            let displays = list_displays(self.language).map_err(|err| {
                self.fill(language::MONITORS_NOT_FOUND, &[("err", &err.message)])
            })?;
            prepare_overlays(self.language, &displays)
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
            action @ (PanelAction::Refresh
            | PanelAction::BeforeAfter
            | PanelAction::Copy
            | PanelAction::Save
            |             PanelAction::Close) => {
                self.windows[index].strip_press = Some(action);
                window.request_redraw();
            }
        }
    }

    fn strip_press_visual(&self, index: usize) -> Option<PanelAction> {
        let (x, y) = self.windows[index].cursor?;
        let (width, height) = logical_client_size(&self.windows[index].window);
        let hit = client_hit(x, y, width, height, self.windows[index].strip_visible);
        strip_press_target(self.windows[index].strip_press, hit)
    }

    fn finish_strip_press(&mut self, index: usize) -> bool {
        let remembered = self.windows[index].strip_press.take();
        let hit = if let Some((x, y)) = self.windows[index].cursor {
            let (width, height) = logical_client_size(&self.windows[index].window);
            client_hit(x, y, width, height, self.windows[index].strip_visible)
        } else {
            PanelAction::Drag
        };
        let action = strip_press_target(remembered, hit);
        if action != Some(PanelAction::Close) && remembered.is_some() {
            self.windows[index].window.request_redraw();
        }
        match action {
            Some(PanelAction::Refresh) => self.refresh(index),
            Some(PanelAction::BeforeAfter) => {
                let number = self.windows[index].number;
                let _ = self.patches.toggle_shown(number);
                self.windows[index].window.request_redraw();
            }
            Some(PanelAction::Copy) => self.copy_shown(index),
            Some(PanelAction::Save) => self.save_shown(index),
            Some(PanelAction::Close) => {
                self.close_patch(index);
                return true;
            }
            _ => {}
        }
        false
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

    fn tray_menu(&self) -> Result<(Menu, TrayMenus), String> {
        let menu = Menu::new();
        let items = vec![
            MenuItem::with_id("new", self.say(language::TRAY_NEW), true, None),
            MenuItem::with_id("show", self.say(language::TRAY_SHOW), true, None),
            MenuItem::with_id("hide", self.say(language::TRAY_HIDE), true, None),
            MenuItem::with_id("close-all", self.say(language::TRAY_CLOSE_ALL), true, None),
            MenuItem::with_id("help", self.say(language::TRAY_HELP), true, None),
            MenuItem::with_id("exit", self.say(language::TRAY_EXIT), true, None),
        ];
        let russian = CheckMenuItem::with_id(
            "lang-ru",
            LANGUAGE_NAME_RU,
            true,
            self.language == Language::Ru,
            None,
        );
        let english = CheckMenuItem::with_id(
            "lang-en",
            LANGUAGE_NAME_EN,
            true,
            self.language == Language::En,
            None,
        );
        let language_menu = Submenu::with_id_and_items(
            "language",
            self.say(language::TRAY_LANGUAGE),
            true,
            &[&russian, &english],
        )
        .map_err(|err| self.fill(language::MENU_FAILED, &[("err", &err.to_string())]))?;
        let above = PredefinedMenuItem::separator();
        let below = PredefinedMenuItem::separator();
        let refs: Vec<&dyn tray_icon::menu::IsMenuItem> = vec![
            &items[0],
            &items[1],
            &items[2],
            &items[3],
            &above,
            &language_menu,
            &items[4],
            &below,
            &items[5],
        ];
        menu.append_items(&refs)
            .map_err(|err| self.fill(language::MENU_FAILED, &[("err", &err.to_string())]))?;
        Ok((
            menu,
            TrayMenus {
                items,
                language_menu,
                russian,
                english,
            },
        ))
    }

    fn replace_tray_menu(&mut self) -> Result<(), String> {
        let (menu, menus) = self.tray_menu()?;
        if let Some(tray) = &self.tray {
            tray.set_menu(Some(Box::new(menu)));
        }
        self.tray_menus = Some(menus);
        Ok(())
    }

    fn choose_language(&mut self, picked: Language) {
        let Some(next) = language_choice(self.language, picked) else {
            self.restore_language_checks();
            return;
        };
        self.language = next;
        if let Err(err) = write_saved_language(self.language) {
            eprintln!(
                "{}",
                self.fill(language::LANGUAGE_SAVE_FAILED, &[("err", &err)])
            );
        }
        if let Err(err) = self.replace_tray_menu() {
            eprintln!("{err}");
        }
        for patch in &self.windows {
            patch.window.request_redraw();
        }
        if let Some(help) = &self.help {
            help.window.set_title(self.say(language::TRAY_HELP));
            help.window.request_redraw();
        }
    }

    fn restore_language_checks(&self) {
        let Some(menus) = &self.tray_menus else {
            return;
        };
        menus.russian.set_checked(self.language == Language::Ru);
        menus.english.set_checked(self.language == Language::En);
    }

    fn install_shell(&mut self) -> Result<(), String> {
        let (menu, menus) = self.tray_menu()?;
        let source = tray_icon_source(self.language)?;
        let icon = Icon::from_rgba(source.pixels, source.width, source.height)
            .map_err(|err| self.fill(language::ICON_CREATE_FAILED, &[("err", &err.to_string())]))?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ScreenPatch")
            .with_icon(icon)
            .build()
            .map_err(|err| self.fill(language::TRAY_CREATE_FAILED, &[("err", &err.to_string())]))?;
        self.tray_menus = Some(menus);
        self.tray = Some(tray);
        match register_hotkey() {
            Ok((manager, id)) => {
                self.hotkeys = Some(manager);
                self.hotkey_id = id;
            }
            Err(err) => eprintln!(
                "{}",
                self.fill(language::HOTKEY_BUSY, &[("err", &err)])
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
        } else if id == "lang-ru" {
            self.choose_language(Language::Ru);
        } else if id == "lang-en" {
            self.choose_language(Language::En);
        } else if id == "help" {
            self.open_help(event_loop);
        } else if id == "exit" {
            self.exit_app(event_loop);
        }
    }

    fn open_help(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(help) = &self.help {
            help.window.set_minimized(false);
            help.window.set_visible(true);
            help.window.focus_window();
            return;
        }
        let displays = match list_displays(self.language) {
            Ok(displays) if !displays.is_empty() => displays,
            Ok(_) => {
                eprintln!("{}", self.say(language::NO_MONITORS));
                return;
            }
            Err(err) => {
                eprintln!("{}", err.message);
                return;
            }
        };
        let Some(index) = help_display_index(&displays) else {
            eprintln!("{}", self.say(language::WORK_AREA_SCALE));
            return;
        };
        let display = &displays[index];
        let (local_x, local_y) = help_window_origin(display.work);
        let Some(offset_x) = logical_to_physical_px(local_x, display.scale) else {
            eprintln!("{}", self.say(language::WORK_AREA_SCALE));
            return;
        };
        let Some(offset_y) = logical_to_physical_px(local_y, display.scale) else {
            eprintln!("{}", self.say(language::WORK_AREA_SCALE));
            return;
        };
        let Some(position_x) = display.origin_x.checked_add(offset_x) else {
            eprintln!("{}", self.say(language::WORK_AREA_SCALE));
            return;
        };
        let Some(position_y) = display.origin_y.checked_add(offset_y) else {
            eprintln!("{}", self.say(language::WORK_AREA_SCALE));
            return;
        };
        let attributes = apply_window_icon(
            self.language,
            WindowAttributes::default()
                .with_title(self.say(language::TRAY_HELP))
                .with_resizable(false)
                .with_inner_size(LogicalSize::new(
                    HELP_CLIENT_WIDTH as f64,
                    HELP_CLIENT_HEIGHT as f64,
                ))
                .with_position(PhysicalPosition::new(position_x, position_y)),
        );
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!(
                    "{}",
                    self.fill(language::WINDOW_CREATE_FAILED, &[("err", &err.to_string())])
                );
                return;
            }
        };
        let context = match softbuffer::Context::new(Arc::clone(&window)) {
            Ok(context) => context,
            Err(err) => {
                eprintln!(
                    "{}",
                    self.fill(language::WINDOW_BUFFER_FAILED, &[("err", &err.to_string())])
                );
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, Arc::clone(&window)) {
            Ok(surface) => surface,
            Err(err) => {
                eprintln!(
                    "{}",
                    self.fill(language::WINDOW_SURFACE_FAILED, &[("err", &err.to_string())])
                );
                return;
            }
        };
        window.request_redraw();
        self.help = Some(HelpWindow {
            window,
            surface,
            context,
        });
    }

    fn help_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.help = None,
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if event.state != ElementState::Pressed || event.repeat || is_synthetic {
                    return;
                }
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    self.help = None;
                }
            }
            WindowEvent::RedrawRequested => self.paint_help_window(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(help) = &self.help {
                    help.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn paint_help_window(&mut self) {
        let Some(window) = self.help.as_ref().map(|help| Arc::clone(&help.window)) else {
            return;
        };
        let size = window.inner_size();
        let pixels = paint_help(
            size.width,
            size.height,
            window.scale_factor(),
            help_lines(self.language),
        );
        let frame = ImageFrame {
            pixels: Arc::from(pixels.into_boxed_slice()),
            width: size.width,
            height: size.height,
        };
        let Some(help) = &mut self.help else {
            return;
        };
        present_frame(&window, &mut help.surface, &frame);
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
        if !confirm_close_all(parent, self.say(language::CLOSE_ALL_QUESTION)) {
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
        self.help = None;
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
            Err(err) => eprintln!(
                "{}",
                self.fill(
                    language::COPY_FAILED,
                    &[("number", &number.to_string()), ("err", &err.to_string())],
                )
            ),
        }
    }

    fn save_shown(&self, index: usize) {
        let number = self.windows[index].number;
        let Some(frame) = self.patches.shown_frame(number).cloned() else {
            return;
        };
        let parent = hwnd_from_window(&self.windows[index].window);
        let path = match choose_png_path(self.language, parent, &suggested_png_name()) {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(err) => {
                eprintln!("{}", self.fill(language::SAVE_FAILED, &[("number", &number.to_string()), ("err", &err.to_string())]));
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
            eprintln!("{}", self.fill(language::SAVE_FAILED, &[("number", &number.to_string()), ("err", &err.to_string())]));
        }
    }
}

struct AppIcon {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

fn app_icon(language: Language) -> Result<&'static AppIcon, String> {
    static ICON: OnceLock<Result<AppIcon, String>> = OnceLock::new();
    ICON.get_or_init(|| {
        let image = image::load_from_memory(include_bytes!("icon.png"))
            .map_err(|err| language::render(language, language::ICON_READ_FAILED, &[("err", &err.to_string())]))?;
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

fn tray_icon_source(language: Language) -> Result<AppIcon, String> {
    let source = app_icon(language)?;
    let image = image::RgbaImage::from_raw(source.width, source.height, source.pixels.clone())
        .ok_or_else(|| language::text(language, language::ICON_DECODE_FAILED).to_string())?;
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

fn apply_window_icon(language: Language, attributes: WindowAttributes) -> WindowAttributes {
    let source = match app_icon(language) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("{err}");
            return attributes;
        }
    };
    match winit::window::Icon::from_rgba(source.pixels.clone(), source.width, source.height) {
        Ok(icon) => attributes.with_window_icon(Some(icon)),
        Err(err) => {
            eprintln!("{}", language::render(language, language::WINDOW_ICON_FAILED, &[("err", &err.to_string())]));
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

fn compose_patch(frame: &ImageFrame, width: u32, height: u32) -> ImageFrame {
    let width = width.max(1);
    let height = height.max(1);
    let mut pixels = vec![24u8, 24, 24, 255].repeat(width as usize * height as usize);
    if let Some(placed) = letterbox(width, height, frame.width, frame.height) {
        for local_y in 0..placed.height {
            let source_y =
                ((u64::from(local_y) * u64::from(frame.height)) / u64::from(placed.height)) as u32;
            for local_x in 0..placed.width {
                let source_x =
                    ((u64::from(local_x) * u64::from(frame.width)) / u64::from(placed.width)) as u32;
                let source = ((source_y * frame.width + source_x) * 4) as usize;
                let destination_y = placed.y + local_y;
                let destination = ((destination_y * width + placed.x + local_x) * 4) as usize;
                pixels[destination..destination + 4]
                    .copy_from_slice(&frame.pixels[source..source + 4]);
            }
        }
    }
    ImageFrame {
        pixels: Arc::from(pixels.into_boxed_slice()),
        width,
        height,
    }
}

fn button_icons(language: Language) -> &'static [Option<ButtonIcon>; 5] {
    static ICONS: OnceLock<[Option<ButtonIcon>; 5]> = OnceLock::new();
    ICONS.get_or_init(|| {
        const BUTTON_ICON_BYTES: [&[u8]; 5] = [
            include_bytes!("icons/refresh.png"),
            include_bytes!("icons/before-after.png"),
            include_bytes!("icons/copy.png"),
            include_bytes!("icons/save.png"),
            include_bytes!("icons/close.png"),
        ];
        std::array::from_fn(|index| match decode_button_icon(BUTTON_ICON_BYTES[index]) {
            Some(icon) => Some(icon),
            None => {
                eprintln!(
                    "{}",
                    language::render(
                        language,
                        language::BUTTON_ICON_FAILED,
                        &[("err", "invalid image")],
                    )
                );
                None
            }
        })
    })
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

fn patch_add_error(language: Language, frame: &ImageFrame, err: AddPatchError) -> String {
    match err {
        AddPatchError::TooMany => language::text(language, language::TOO_MANY_SHORT).to_string(),
        AddPatchError::OverBudget {
            frame_bytes,
            used_bytes,
        } => language::render(
            language,
            language::FRAME_BUDGET,
            &[
                ("w", &frame.width.to_string()),
                ("h", &frame.height.to_string()),
                ("bytes", &frame_bytes.to_string()),
                ("used", &used_bytes.to_string()),
            ],
        ),
    }
}

fn absolute_work(display: &DisplayInfo) -> Option<(i32, i32, u32, u32)> {
    let x = display
        .origin_x
        .checked_add(logical_to_physical_px(display.work.x, display.scale)?)?;
    let y = display
        .origin_y
        .checked_add(logical_to_physical_px(display.work.y, display.scale)?)?;
    let width = u32::try_from(logical_to_physical_px(
        i32::try_from(display.work.width).ok()?,
        display.scale,
    )?)
    .ok()?;
    let height = u32::try_from(logical_to_physical_px(
        i32::try_from(display.work.height).ok()?,
        display.scale,
    )?)
    .ok()?;
    Some((x, y, width, height))
}

fn help_display_index(displays: &[DisplayInfo]) -> Option<usize> {
    let mut placed = Vec::new();
    for (index, display) in displays.iter().enumerate() {
        if let Some(rect) = absolute_work(display) {
            placed.push((index, rect));
        }
    }
    if placed.is_empty() {
        return None;
    }
    let rects: Vec<_> = placed.iter().map(|(_, rect)| *rect).collect();
    if let Some((x, y)) = cursor_screen_point() {
        if let Some(hit) = index_containing_point(x, y, &rects) {
            return Some(placed[hit].0);
        }
    }
    if absolute_work(&displays[0]).is_some() {
        Some(0)
    } else {
        Some(placed[0].0)
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

fn physical_to_logical_i32(language: Language, physical: i32, scale: f64) -> Result<i32, String> {
    screenpatch::core::geometry::physical_to_logical_px(physical, scale)
        .ok_or_else(|| language::text(language, language::BAD_SCALE_SHORT).to_string())
}

fn log_monitor(language: Language, display: &DisplayInfo, region: CaptureRect, affinity: bool) {
    let exclusion = language::text(
        language,
        if affinity {
            language::EXCLUDED_AFFINITY
        } else {
            language::EXCLUDED_HIDDEN
        },
    );
    eprintln!(
        "{}",
        language::render(
            language,
            language::MONITOR_LOG,
            &[
                ("id", &display.id),
                ("x", &display.origin_x.to_string()),
                ("y", &display.origin_y.to_string()),
                ("w", &display.physical_width.to_string()),
                ("h", &display.physical_height.to_string()),
                ("scale", &format_scale(display.scale)),
                ("rx", &region.x.to_string()),
                ("ry", &region.y.to_string()),
                ("rw", &region.width.to_string()),
                ("rh", &region.height.to_string()),
                ("excluded", exclusion),
            ],
        )
    );
}

fn log_refresh_cancel(language: Language, monitor_id: &str, region: CaptureRect, reason: &str) {
    eprintln!(
        "{}",
        language::render(
            language,
            language::REFRESH_CANCELLED,
            &[
                ("reason", reason),
                ("id", monitor_id),
                ("x", &region.x.to_string()),
                ("y", &region.y.to_string()),
                ("w", &region.width.to_string()),
                ("h", &region.height.to_string()),
            ],
        )
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
