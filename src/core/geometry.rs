#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct MonitorCandidate {
    pub scale: f64,
    pub physical_width: u32,
    pub physical_height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchGeometry {
    pub region: CaptureRect,
    pub window_x: i32,
    pub window_y: i32,
}

impl PatchGeometry {
    pub fn move_to(&mut self, x: i32, y: i32) {
        self.window_x = x;
        self.window_y = y;
    }
}

pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
const MARGIN: u32 = 64;
const MIN_SIDE: u32 = 16;
const PLACE_GAP: i32 = 16;

pub fn global_to_monitor_local(
    global_x: i32,
    global_y: i32,
    origin_x: i32,
    origin_y: i32,
) -> (i32, i32) {
    (
        global_x.wrapping_sub(origin_x),
        global_y.wrapping_sub(origin_y),
    )
}

pub fn logical_window_size(
    physical_width: u32,
    physical_height: u32,
    scale: f64,
) -> Option<(u32, u32)> {
    Some((
        logical_side(physical_width, scale)?,
        logical_side(physical_height, scale)?,
    ))
}

pub fn capture_rect_for_monitor(physical_width: u32, physical_height: u32) -> Option<CaptureRect> {
    let max_w = physical_width.checked_sub(MARGIN.saturating_mul(2))?;
    let max_h = physical_height.checked_sub(MARGIN.saturating_mul(2))?;
    if max_w < MIN_SIDE || max_h < MIN_SIDE {
        return None;
    }
    Some(CaptureRect {
        x: MARGIN as i32,
        y: MARGIN as i32,
        width: 640.min(max_w),
        height: 400.min(max_h),
    })
}

pub fn frame_byte_len(width: u32, height: u32) -> Option<usize> {
    let bytes = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if bytes > MAX_FRAME_BYTES {
        None
    } else {
        Some(bytes)
    }
}

pub fn place_window(
    capture: LogicalRect,
    window_width: u32,
    window_height: u32,
    work: WorkArea,
) -> (i32, i32) {
    let right = (capture.x + capture.width as i32 + PLACE_GAP, capture.y);
    if fits(work, right, window_width, window_height)
        && !intersects(capture, right, window_width, window_height)
    {
        return right;
    }

    let below = (capture.x, capture.y + capture.height as i32 + PLACE_GAP);
    if fits(work, below, window_width, window_height)
        && !intersects(capture, below, window_width, window_height)
    {
        return below;
    }

    farthest_corner(capture, window_width, window_height, work)
}

pub fn select_monitor_pair(monitors: &[MonitorCandidate]) -> Option<(usize, usize)> {
    let fitting: Vec<usize> = monitors
        .iter()
        .enumerate()
        .filter(|(_, monitor)| {
            capture_rect_for_monitor(monitor.physical_width, monitor.physical_height).is_some()
        })
        .map(|(index, _)| index)
        .collect();

    for (offset, &first) in fitting.iter().enumerate() {
        for &second in &fitting[offset + 1..] {
            if !scales_match(monitors[first].scale, monitors[second].scale) {
                return Some((first, second));
            }
        }
    }
    None
}

pub fn scales_match(left: f64, right: f64) -> bool {
    (left - right).abs() < 0.001
}

pub fn physical_to_logical_px(physical: i32, scale: f64) -> Option<i32> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    Some(round_half_away_from_zero(physical as f64 / scale))
}

pub fn logical_to_physical_px(logical: i32, scale: f64) -> Option<i32> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    Some(round_half_away_from_zero(logical as f64 * scale))
}

fn round_half_away_from_zero(value: f64) -> i32 {
    let magnitude = value.abs();
    let fraction = magnitude - magnitude.floor();
    let rounded = if fraction >= 0.5 {
        magnitude.floor() + 1.0
    } else {
        magnitude.floor()
    };
    let signed = if value < 0.0 { -rounded } else { rounded };
    signed as i32
}

fn logical_side(physical: u32, scale: f64) -> Option<u32> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let value = physical as f64 / scale;
    let fraction = value - value.floor();
    let rounded = if fraction >= 0.5 {
        value.floor() + 1.0
    } else {
        value.floor()
    };
    Some((rounded as u32).max(1))
}

fn fits(work: WorkArea, origin: (i32, i32), width: u32, height: u32) -> bool {
    let (x, y) = origin;
    x >= work.x
        && y >= work.y
        && i64::from(x) + i64::from(width) <= i64::from(work.x) + i64::from(work.width)
        && i64::from(y) + i64::from(height) <= i64::from(work.y) + i64::from(work.height)
}

fn intersects(capture: LogicalRect, origin: (i32, i32), width: u32, height: u32) -> bool {
    let (x, y) = origin;
    x < capture.x + capture.width as i32
        && x + width as i32 > capture.x
        && y < capture.y + capture.height as i32
        && y + height as i32 > capture.y
}

fn farthest_corner(
    capture: LogicalRect,
    window_width: u32,
    window_height: u32,
    work: WorkArea,
) -> (i32, i32) {
    if window_width > work.width || window_height > work.height {
        return (work.x, work.y);
    }
    let corners = [
        (work.x, work.y),
        (
            work.x + work.width as i32 - window_width as i32,
            work.y,
        ),
        (
            work.x,
            work.y + work.height as i32 - window_height as i32,
        ),
        (
            work.x + work.width as i32 - window_width as i32,
            work.y + work.height as i32 - window_height as i32,
        ),
    ];
    corners
        .into_iter()
        .max_by_key(|&(x, y)| corner_distance(capture, window_width, window_height, x, y))
        .unwrap_or((work.x, work.y))
}

fn corner_distance(
    capture: LogicalRect,
    window_width: u32,
    window_height: u32,
    x: i32,
    y: i32,
) -> i64 {
    let window_cx = i64::from(x) * 2 + i64::from(window_width);
    let window_cy = i64::from(y) * 2 + i64::from(window_height);
    let capture_cx = i64::from(capture.x) * 2 + i64::from(capture.width);
    let capture_cy = i64::from(capture.y) * 2 + i64::from(capture.height);
    let dx = window_cx - capture_cx;
    let dy = window_cy - capture_cy;
    dx * dx + dy * dy
}
