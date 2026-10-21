use std::num::NonZeroU32;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

use screenpatch::core::geometry::{
    capture_rect_for_monitor, logical_to_physical_px, logical_window_size, place_window,
    select_monitor_pair, CaptureRect, LogicalRect, MonitorCandidate,
};
use screenpatch::core::refresh::{fingerprints_match, refresh_current, ImageFrame};
use screenpatch::platform::{
    capture_monitor_region, hwnd_from_window, list_displays, try_exclude_from_capture,
    wait_until_hidden, DisplayInfo, Hwnd,
};

struct PreparedWindow {
    display: DisplayInfo,
    region: CaptureRect,
    frame: ImageFrame,
}

struct PatchWindow {
    window: Arc<Window>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    // Context должен пережить Surface: поля сбрасываются в порядке объявления.
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
    prepared: Option<Vec<PreparedWindow>>,
    windows: Vec<PatchWindow>,
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
    let prepared = prepare_windows(&displays)?;
    let event_loop = EventLoop::new().map_err(|err| format!("не удалось создать цикл событий: {err}"))?;
    let mut app = App {
        prepared: Some(prepared),
        windows: Vec::new(),
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

fn prepare_windows(displays: &[DisplayInfo]) -> Result<Vec<PreparedWindow>, String> {
    let candidates: Vec<MonitorCandidate> = displays
        .iter()
        .map(|display| MonitorCandidate {
            scale: display.scale,
            physical_width: display.physical_width,
            physical_height: display.physical_height,
        })
        .collect();
    let Some((first, second)) = select_monitor_pair(&candidates) else {
        return Err(
            "не найдены два монитора с разным масштабом, пригодные для захвата".to_string(),
        );
    };
    let mut prepared = Vec::new();
    for index in [first, second] {
        let display = &displays[index];
        let region = capture_rect_for_monitor(display.physical_width, display.physical_height)
            .ok_or_else(|| {
                format!(
                    "для монитора {} нельзя построить прямоугольник захвата",
                    display.id
                )
            })?;
        let frame = capture_monitor_region(&display.id, region).map_err(|err| {
            format!(
                "захват монитора {} не удался: {}",
                display.id, err.message
            )
        })?;
        prepared.push(PreparedWindow {
            display: display.clone(),
            region,
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
        if let Err(err) = self.open_windows(event_loop) {
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
            WindowEvent::RedrawRequested => self.paint(index),
            _ => {}
        }
    }
}

impl App {
    fn open_windows(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let prepared = self
            .prepared
            .take()
            .ok_or_else(|| "окна уже созданы".to_string())?;
        for item in prepared {
            let (logical_w, logical_h) =
                logical_window_size(item.frame.width, item.frame.height, item.display.scale)
                    .ok_or_else(|| {
                        format!("масштаб монитора {} некорректен", item.display.id)
                    })?;
            let capture = LogicalRect {
                x: physical_to_logical_i32(item.region.x, item.display.scale)?,
                y: physical_to_logical_i32(item.region.y, item.display.scale)?,
                width: logical_w,
                height: logical_h,
            };
            let (local_x, local_y) = place_window(capture, logical_w, logical_h, item.display.work);
            let position_x = item.display.origin_x
                + logical_to_physical_px(local_x, item.display.scale).ok_or_else(|| {
                    format!("не удалось перевести положение окна монитора {}", item.display.id)
                })?;
            let position_y = item.display.origin_y
                + logical_to_physical_px(local_y, item.display.scale).ok_or_else(|| {
                    format!("не удалось перевести положение окна монитора {}", item.display.id)
                })?;
            let attributes = WindowAttributes::default()
                .with_title(format!("ScreenPatch {}", item.display.id))
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
            let hwnd = hwnd_from_window(&window)
                .ok_or_else(|| "у окна нет дескриптора Win32".to_string())?;
            let affinity = try_exclude_from_capture(hwnd);
            log_monitor(&item.display, item.region, affinity);
            window.request_redraw();
            self.windows.push(PatchWindow {
                window,
                context,
                surface,
                region: item.region,
                frame: item.frame,
                fingerprint_id: item.display.id.clone(),
                origin_x: item.display.origin_x,
                origin_y: item.display.origin_y,
                physical_width: item.display.physical_width,
                physical_height: item.display.physical_height,
                scale: item.display.scale,
                affinity,
                refreshing: false,
            });
        }
        Ok(())
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
            log_refresh_cancel(
                &monitor_id,
                region,
                "монитор больше не подключён",
            );
            return;
        };
        if !fingerprints_match(&saved, &display.fingerprint()) {
            log_refresh_cancel(
                &monitor_id,
                region,
                "геометрия монитора изменилась",
            );
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

    fn paint(&mut self, index: usize) {
        let patch = &mut self.windows[index];
        let size = patch.window.inner_size();
        let Some(width) = NonZeroU32::new(size.width) else {
            return;
        };
        let Some(height) = NonZeroU32::new(size.height) else {
            return;
        };
        if patch.surface.resize(width, height).is_err() {
            return;
        }
        let mut buffer = match patch.surface.buffer_mut() {
            Ok(buffer) => buffer,
            Err(_) => return,
        };
        buffer.fill(0x0018_1818);
        blit_frame(&mut buffer, size.width as usize, &patch.frame);
        let _ = buffer.present();
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
