#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelAction {
    Refresh,
    BeforeAfter,
    Copy,
    Save,
    Close,
    Drag,
    Resize,
}

const RESIZE_GRIP: i32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonRect {
    pub action: PanelAction,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

const BUTTON_ACTIONS: [PanelAction; 5] = [
    PanelAction::Refresh,
    PanelAction::BeforeAfter,
    PanelAction::Copy,
    PanelAction::Save,
    PanelAction::Close,
];

pub fn button_rects(width: u32, _height: u32) -> [ButtonRect; 5] {
    let (button_w, gap) = button_metrics(width);
    let group = button_w.saturating_mul(5).saturating_add(gap.saturating_mul(4));
    let origin = (width as i32).saturating_sub(group as i32) / 2;
    std::array::from_fn(|index| ButtonRect {
        action: BUTTON_ACTIONS[index],
        x: origin + (button_w.saturating_add(gap) as i32) * index as i32,
        y: 8,
        width: button_w,
        height: 30,
    })
}

fn button_metrics(width: u32) -> (u32, u32) {
    if width >= 5 * 44 + 4 * 8 + 2 * 8 {
        return (44, 8);
    }
    let inner = width.saturating_sub(16);
    let button_w = inner.saturating_sub(16) / 5;
    if button_w >= 28 {
        (button_w, 4)
    } else {
        (28, 2)
    }
}

pub fn client_hit(x: i32, y: i32, width: u32, height: u32, buttons_visible: bool) -> PanelAction {
    if width == 0 || height == 0 || x < 0 || y < 0 {
        return PanelAction::Drag;
    }
    let Ok(width_i) = i32::try_from(width) else {
        return PanelAction::Drag;
    };
    let Ok(height_i) = i32::try_from(height) else {
        return PanelAction::Drag;
    };
    if x >= width_i || y >= height_i {
        return PanelAction::Drag;
    }
    if x >= width_i - RESIZE_GRIP && y >= height_i - RESIZE_GRIP {
        return PanelAction::Resize;
    }
    if !buttons_visible {
        return PanelAction::Drag;
    }
    for rect in button_rects(width, height) {
        if point_in_rounded_rect(x, y, rect.x, rect.y, rect.width, rect.height, 8) {
            return rect.action;
        }
    }
    PanelAction::Drag
}

pub fn point_in_rounded_rect(
    x: i32,
    y: i32,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    radius: i32,
) -> bool {
    let Ok(width) = i32::try_from(width) else {
        return false;
    };
    let Ok(height) = i32::try_from(height) else {
        return false;
    };
    let right = left.saturating_add(width);
    let bottom = top.saturating_add(height);
    if x < left || y < top || x >= right || y >= bottom {
        return false;
    }
    let radius = radius.max(0).min(width / 2).min(height / 2);
    let (cx, cy) = if x < left + radius && y < top + radius {
        (left + radius, top + radius)
    } else if x >= right - radius && y < top + radius {
        (right - radius, top + radius)
    } else if x < left + radius && y >= bottom - radius {
        (left + radius, bottom - radius)
    } else if x >= right - radius && y >= bottom - radius {
        (right - radius, bottom - radius)
    } else {
        return true;
    };
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= radius * radius
}

pub fn strip_press_target(remembered: Option<PanelAction>, hit: PanelAction) -> Option<PanelAction> {
    match remembered {
        Some(
            action @ (PanelAction::Refresh
            | PanelAction::BeforeAfter
            | PanelAction::Copy
            | PanelAction::Save
            | PanelAction::Close),
        ) if hit == action => Some(action),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct ButtonIcon {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub fn draw_buttons(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    scale: f64,
    pressed: Option<PanelAction>,
    icons: &[Option<ButtonIcon>; 5],
) {
    use crate::core::geometry::{logical_client_keeping_physical, logical_to_physical_px};
    let Some((logical_w, logical_h)) = logical_client_keeping_physical(width, height, scale) else {
        return;
    };
    let scale_len = |logical: u32| -> u32 {
        u32::try_from(
            logical_to_physical_px(i32::try_from(logical).unwrap_or(i32::MAX), scale).unwrap_or(0),
        )
        .unwrap_or(0)
    };
    let scale_pos = |logical: i32| logical_to_physical_px(logical, scale).unwrap_or(0);
    let radius = scale_len(8).max(1) as i32;
    for (index, rect) in button_rects(logical_w, logical_h).into_iter().enumerate() {
        let left = scale_pos(rect.x);
        let top = scale_pos(rect.y);
        let button_w = scale_len(rect.width);
        let button_h = scale_len(rect.height);
        let fill = if pressed == Some(rect.action) {
            [14, 14, 14, 220]
        } else {
            [32, 32, 32, 170]
        };
        for y in top..top.saturating_add(button_h as i32) {
            for x in left..left.saturating_add(button_w as i32) {
                if point_in_rounded_rect(x, y, left, top, button_w, button_h, radius) {
                    blend_pixel(pixels, width, height, x, y, fill);
                }
            }
        }
        let Some(icon) = icons[index].as_ref() else {
            continue;
        };
        if icon.width == 0 || icon.height == 0 {
            continue;
        }
        let icon_w = scale_len(16).max(1);
        let icon_h = scale_len(16).max(1);
        let icon_left = left + (button_w as i32 - icon_w as i32) / 2;
        let icon_top = top + (button_h as i32 - icon_h as i32) / 2;
        for y in 0..icon_h {
            for x in 0..icon_w {
                blend_pixel(
                    pixels,
                    width,
                    height,
                    icon_left + x as i32,
                    icon_top + y as i32,
                    sample_icon(icon, x, y, icon_w, icon_h),
                );
            }
        }
    }
}

pub fn decode_button_icon(bytes: &[u8]) -> Option<ButtonIcon> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    Some(ButtonIcon {
        width,
        height,
        pixels: image.into_raw(),
    })
}

fn sample_span(dest: u32, dest_size: u32, source_size: u32) -> (u32, u32) {
    if dest_size == 0 || source_size == 0 {
        return (0, 0);
    }
    let mut start = (u64::from(dest) * u64::from(source_size) / u64::from(dest_size)) as u32;
    let mut end = (u64::from(dest + 1) * u64::from(source_size) / u64::from(dest_size)) as u32;
    if end <= start {
        end = start.saturating_add(1);
    }
    if start >= source_size {
        start = source_size - 1;
        end = source_size;
    } else if end > source_size {
        end = source_size;
    }
    (start, end)
}

fn sample_icon(icon: &ButtonIcon, x: u32, y: u32, dest_w: u32, dest_h: u32) -> [u8; 4] {
    let (x0, x1) = sample_span(x, dest_w, icon.width);
    let (y0, y1) = sample_span(y, dest_h, icon.height);
    let mut alpha_sum = 0u32;
    let mut count = 0u32;
    for sy in y0..y1 {
        for sx in x0..x1 {
            let index = ((sy * icon.width + sx) * 4) as usize;
            if index + 4 > icon.pixels.len() {
                continue;
            }
            alpha_sum += u32::from(icon.pixels[index + 3]);
            count += 1;
        }
    }
    if count == 0 {
        return [255, 255, 255, 0];
    }
    [255, 255, 255, (alpha_sum / count) as u8]
}

fn blend_pixel(pixels: &mut [u8], width: u32, height: u32, x: i32, y: i32, src: [u8; 4]) {
    if x < 0 || y < 0 {
        return;
    }
    let Ok(x) = u32::try_from(x) else {
        return;
    };
    let Ok(y) = u32::try_from(y) else {
        return;
    };
    if x >= width || y >= height {
        return;
    }
    let index = ((y * width + x) * 4) as usize;
    if index + 4 > pixels.len() {
        return;
    }
    let alpha = u32::from(src[3]);
    let inverse = 255 - alpha;
    for channel in 0..3 {
        let mixed = u32::from(src[channel]) * alpha + u32::from(pixels[index + channel]) * inverse;
        pixels[index + channel] = (mixed / 255) as u8;
    }
    pixels[index + 3] = 255;
}
