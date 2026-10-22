use crate::core::geometry::{frame_byte_len, CaptureRect};
use crate::core::language::{self, Language};
use crate::core::refresh::{image_frame, ImageFrame};

#[derive(Debug)]
pub struct CaptureError {
    pub message: String,
}

impl CaptureError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait ScreenCapture {
    fn capture_region(&mut self, region: &CaptureRect) -> Result<ImageFrame, CaptureError>;
}

pub fn frame_from_rgba(
    language: Language,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
) -> Result<ImageFrame, CaptureError> {
    if frame_byte_len(width, height).is_none() {
        return Err(CaptureError::new(language::text(language, language::FRAME_TOO_BIG)));
    }
    image_frame(width, height, pixels).ok_or_else(|| {
        CaptureError::new(language::text(language, language::FRAME_BUFFER_MISMATCH))
    })
}
