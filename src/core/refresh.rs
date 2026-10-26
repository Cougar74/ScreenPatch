use std::sync::Arc;

use crate::core::geometry::scales_match;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageFrame {
    pub pixels: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fingerprint {
    pub id: String,
    pub origin_x: i32,
    pub origin_y: i32,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale: f64,
}

pub fn image_frame(width: u32, height: u32, pixels: Vec<u8>) -> Option<ImageFrame> {
    let expected = crate::core::geometry::frame_byte_len(width, height)?;
    if pixels.len() != expected {
        return None;
    }
    Some(ImageFrame {
        pixels: Arc::from(pixels),
        width,
        height,
    })
}

pub fn fingerprints_match(saved: &Fingerprint, current: &Fingerprint) -> bool {
    saved.id == current.id
        && saved.origin_x == current.origin_x
        && saved.origin_y == current.origin_y
        && saved.physical_width == current.physical_width
        && saved.physical_height == current.physical_height
        && scales_match(saved.scale, current.scale)
}

pub fn refresh_current(
    current: ImageFrame,
    fingerprint_matches: bool,
    hide_before_capture: bool,
    capture: impl FnOnce() -> Result<ImageFrame, ()>,
    restore_visibility: impl FnOnce(),
) -> ImageFrame {
    if !fingerprint_matches {
        return current;
    }
    let captured = if hide_before_capture {
        with_visibility_restore(restore_visibility, capture)
    } else {
        capture()
    };
    captured.unwrap_or(current)
}

fn with_visibility_restore<T>(
    restore_visibility: impl FnOnce(),
    body: impl FnOnce() -> T,
) -> T {
    struct Restore<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for Restore<F> {
        fn drop(&mut self) {
            if let Some(restore) = self.0.take() {
                restore();
            }
        }
    }
    let _guard = Restore(Some(restore_visibility));
    body()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshBlock {
    MonitorMissing,
    GeometryChanged,
}

pub fn refresh_gate(saved: &Fingerprint, monitors: &[Fingerprint]) -> Result<(), RefreshBlock> {
    match monitors.iter().find(|monitor| monitor.id == saved.id) {
        None => Err(RefreshBlock::MonitorMissing),
        Some(monitor) if fingerprints_match(saved, monitor) => Ok(()),
        Some(_) => Err(RefreshBlock::GeometryChanged),
    }
}
