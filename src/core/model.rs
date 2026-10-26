use std::sync::Arc;
use std::time::SystemTime;

use crate::core::geometry::{frame_byte_len, CaptureRect, MAX_FRAME_BYTES};
use crate::core::refresh::Fingerprint;

#[derive(Clone, Debug, PartialEq)]
pub enum CaptureSource {
    Region { rect: CaptureRect, fingerprint: Fingerprint },
    Window,
    Import,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureFrame {
    pub pixels: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
    pub captured_at: SystemTime,
    pub source: CaptureSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShownFrame {
    Current,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureView {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub opacity: u8,
    pub shown: ShownFrame,
}

pub struct CapturePolicy {
    pub max_captures: usize,
    pub byte_budget: usize,
    pub max_frame_bytes: usize,
}

pub fn default_policy() -> CapturePolicy {
    CapturePolicy {
        max_captures: 10,
        byte_budget: 512 * 1024 * 1024,
        max_frame_bytes: MAX_FRAME_BYTES,
    }
}

pub fn frame_accepted(width: u32, height: u32, pixels: usize, policy: &CapturePolicy) -> bool {
    match frame_byte_len(width, height) {
        Some(expected) if expected == pixels && pixels <= policy.max_frame_bytes => true,
        _ => false,
    }
}
