use crate::core::geometry::CaptureRect;
use crate::core::refresh::{image_frame, ImageFrame};

#[derive(Clone, Debug, PartialEq)]
pub struct MonitorReadiness {
    pub id: String,
    pub scale: f64,
    pub physical_width: u32,
    pub physical_height: u32,
    pub frame_ready: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionAbort {
    NoMonitors,
    BadScale { id: String },
    EmptySize { id: String },
    CaptureFailed { id: String },
}

pub fn selection_ready(monitors: &[MonitorReadiness]) -> Result<(), SelectionAbort> {
    if monitors.is_empty() {
        return Err(SelectionAbort::NoMonitors);
    }
    for monitor in monitors {
        if !monitor.scale.is_finite() || monitor.scale <= 0.0 {
            return Err(SelectionAbort::BadScale {
                id: monitor.id.clone(),
            });
        }
        if monitor.physical_width == 0 || monitor.physical_height == 0 {
            return Err(SelectionAbort::EmptySize {
                id: monitor.id.clone(),
            });
        }
        if !monitor.frame_ready {
            return Err(SelectionAbort::CaptureFailed {
                id: monitor.id.clone(),
            });
        }
    }
    Ok(())
}

const GLYPH_SCALE: i32 = 2;
const GLYPH_GAP: i32 = 2;
const LABEL_PAD: i32 = 2;
const LABEL_OFFSET: i32 = 8;

pub fn logical_cursor_to_physical(
    logical_x: f64,
    logical_y: f64,
    scale: f64,
    monitor_width: u32,
    monitor_height: u32,
) -> Option<(i32, i32)> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    Some(clamp_physical_point(
        round_half_away(logical_x * scale),
        round_half_away(logical_y * scale),
        monitor_width,
        monitor_height,
    ))
}

pub fn physical_cursor_point(
    x: f64,
    y: f64,
    monitor_width: u32,
    monitor_height: u32,
) -> (i32, i32) {
    clamp_physical_point(
        round_half_away(x),
        round_half_away(y),
        monitor_width,
        monitor_height,
    )
}

pub fn clamp_physical_point(x: i32, y: i32, monitor_width: u32, monitor_height: u32) -> (i32, i32) {
    (
        x.clamp(0, monitor_width as i32),
        y.clamp(0, monitor_height as i32),
    )
}

pub fn normalize_drag(anchor_x: i32, anchor_y: i32, current_x: i32, current_y: i32) -> CaptureRect {
    CaptureRect {
        x: anchor_x.min(current_x),
        y: anchor_y.min(current_y),
        width: anchor_x.abs_diff(current_x),
        height: anchor_y.abs_diff(current_y),
    }
}

pub fn selection_accepted(rect: CaptureRect) -> bool {
    rect.width >= 16 && rect.height >= 16
}

pub fn crop_frame(frame: &ImageFrame, rect: CaptureRect) -> Option<ImageFrame> {
    if rect.x < 0 || rect.y < 0 {
        return None;
    }
    let right = i64::from(rect.x) + i64::from(rect.width);
    let bottom = i64::from(rect.y) + i64::from(rect.height);
    if right > i64::from(frame.width) || bottom > i64::from(frame.height) {
        return None;
    }
    let mut pixels = Vec::with_capacity((rect.width as usize) * (rect.height as usize) * 4);
    for row in 0..rect.height {
        let y = rect.y as u32 + row;
        let start = ((y * frame.width + rect.x as u32) * 4) as usize;
        let end = start + (rect.width as usize) * 4;
        pixels.extend_from_slice(&frame.pixels[start..end]);
    }
    image_frame(rect.width, rect.height, pixels)
}

pub fn label_box(
    text: &str,
    rect: CaptureRect,
    monitor_width: u32,
    monitor_height: u32,
) -> (i32, i32, u32, u32) {
    let (width, height) = backing_size(text);
    let above = rect.y - LABEL_OFFSET - height as i32;
    let y = if above >= 0 {
        above
    } else {
        let below = rect.y + rect.height as i32 + LABEL_OFFSET;
        if below + height as i32 <= monitor_height as i32 {
            below
        } else {
            rect.y
        }
    };
    let _ = monitor_width;
    (rect.x, y, width, height)
}

pub fn compose_overlay(source: &ImageFrame, selection: Option<CaptureRect>) -> ImageFrame {
    let mut pixels = source.pixels.to_vec();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[0] /= 2;
        pixel[1] /= 2;
        pixel[2] /= 2;
        pixel[3] = 255;
    }
    if let Some(rect) = selection {
        copy_source_rect(source, &mut pixels, rect);
        draw_border(source.width, source.height, &mut pixels, rect);
        let text = format!("{} × {}", rect.width, rect.height);
        draw_label(source.width, source.height, &mut pixels, &text, rect);
    }
    image_frame(source.width, source.height, pixels).unwrap_or_else(|| source.clone())
}

fn copy_source_rect(source: &ImageFrame, destination: &mut [u8], rect: CaptureRect) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    for row in 0..rect.height {
        let y = rect.y as u32 + row;
        if y >= source.height {
            break;
        }
        for column in 0..rect.width {
            let x = rect.x as u32 + column;
            if x >= source.width {
                break;
            }
            let index = ((y * source.width + x) * 4) as usize;
            destination[index..index + 4].copy_from_slice(&source.pixels[index..index + 4]);
        }
    }
}

fn draw_border(width: u32, height: u32, pixels: &mut [u8], rect: CaptureRect) {
    let left = rect.x - 1;
    let right = rect.x + rect.width as i32;
    let top = rect.y - 1;
    let bottom = rect.y + rect.height as i32;
    for x in left..=right {
        plot(width, height, pixels, x, top, [255, 255, 255, 255]);
        plot(width, height, pixels, x, bottom, [255, 255, 255, 255]);
    }
    for y in rect.y..bottom {
        plot(width, height, pixels, left, y, [255, 255, 255, 255]);
        plot(width, height, pixels, right, y, [255, 255, 255, 255]);
    }
}

fn draw_label(width: u32, height: u32, pixels: &mut [u8], text: &str, rect: CaptureRect) {
    let (box_x, box_y, box_w, box_h) = label_box(text, rect, width, height);
    for y in box_y..box_y + box_h as i32 {
        for x in box_x..box_x + box_w as i32 {
            plot(width, height, pixels, x, y, [0, 0, 0, 255]);
        }
    }
    let mut cursor = box_x + LABEL_PAD;
    let glyph_y = box_y + LABEL_PAD;
    for character in text.chars() {
        draw_glyph(width, height, pixels, cursor, glyph_y, character);
        cursor += glyph_span();
    }
}

fn draw_glyph(width: u32, height: u32, pixels: &mut [u8], origin_x: i32, origin_y: i32, character: char) {
    let Some(rows) = glyph_rows(character) else {
        return;
    };
    for (row_index, row) in rows.iter().enumerate() {
        for column in 0..5 {
            if row & (1 << (4 - column)) == 0 {
                continue;
            }
            let pixel_x = origin_x + column * GLYPH_SCALE;
            let pixel_y = origin_y + row_index as i32 * GLYPH_SCALE;
            for dy in 0..GLYPH_SCALE {
                for dx in 0..GLYPH_SCALE {
                    plot(
                        width,
                        height,
                        pixels,
                        pixel_x + dx,
                        pixel_y + dy,
                        [255, 255, 255, 255],
                    );
                }
            }
        }
    }
}

fn backing_size(text: &str) -> (u32, u32) {
    let count = text.chars().count() as i32;
    let text_width = if count == 0 {
        0
    } else {
        count * 5 * GLYPH_SCALE + (count - 1) * GLYPH_GAP
    };
    let text_height = 7 * GLYPH_SCALE;
    (
        (text_width + LABEL_PAD * 2) as u32,
        (text_height + LABEL_PAD * 2) as u32,
    )
}

fn glyph_span() -> i32 {
    5 * GLYPH_SCALE + GLYPH_GAP
}

fn plot(width: u32, height: u32, pixels: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let index = ((y as u32 * width + x as u32) * 4) as usize;
    pixels[index..index + 4].copy_from_slice(&color);
}

fn round_half_away(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
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

fn glyph_rows(character: char) -> Option<[u8; 7]> {
    Some(match character {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '×' => [0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b00000],
        _ => return None,
    })
}
