use std::path::PathBuf;
use std::sync::Arc;

use crate::core::language::{self, Language};
use crate::core::model::{
    default_policy, frame_accepted, CaptureFrame, CapturePolicy, CaptureSource, CaptureView, ShownFrame,
};
use crate::core::port::PlatformPort;
use crate::core::refresh::{refresh_gate, RefreshBlock};

pub struct HeldCapture {
    pub number: u64,
    pub source: CaptureSource,
    pub current: CaptureFrame,
    pub previous: Option<CaptureFrame>,
    pub view: CaptureView,
}

pub struct Session<P: PlatformPort> {
    language: Language,
    next_number: u64,
    captures: Vec<HeldCapture>,
    policy: CapturePolicy,
    port: std::marker::PhantomData<P>,
}

pub enum Request<W> {
    Snapshot { hide: Vec<W> },
    CaptureNew {
        source: CaptureSource,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    CaptureRefresh { number: u64, hide: Vec<W> },
    WindowMoved { number: u64, x: i32, y: i32, width: u32, height: u32 },
    ShownToggled { number: u64 },
    Copy { number: u64 },
    Save { number: u64, path: PathBuf },
    Close { number: u64 },
    LanguageChosen { language: Language },
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    SnapshotSucceeded { monitors: Vec<(crate::core::refresh::Fingerprint, Vec<u8>, u32, u32)> },
    SnapshotFailed { message: String },
    CaptureSucceeded { number: u64 },
    CaptureFailed { message: String },
    SourceUnavailable { message: String },
    ViewUpdated,
    Ignored,
    Closed,
    LanguageSaved { message: Option<String> },
    CopySucceeded,
    CopyFailed { message: String },
    SaveSucceeded,
    SaveFailed { message: String },
}

impl<P: PlatformPort> Session<P> {
    pub fn open(port: &P) -> (Self, Option<String>) {
        let (language, message) = match port.read_language() {
            Ok(Some(language)) => (language, None),
            Ok(None) => (port.system_language(), None),
            Err(err) => {
                let language = port.system_language();
                (
                    language,
                    Some(language::render(
                        language,
                        language::LANGUAGE_READ_FAILED,
                        &[("err", &err)],
                    )),
                )
            }
        };
        (
            Self {
                language,
                next_number: 1,
                captures: Vec::new(),
                policy: default_policy(),
                port: std::marker::PhantomData,
            },
            message,
        )
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn capture(&self, number: u64) -> Option<&HeldCapture> {
        self.captures.iter().find(|capture| capture.number == number)
    }

    pub fn len(&self) -> usize {
        self.captures.len()
    }

    pub fn handle(&mut self, port: &mut P, request: Request<P::Window>) -> Outcome {
        match request {
            Request::Snapshot { hide } => match port.snapshot(&hide) {
                Ok(frames) => Outcome::SnapshotSucceeded {
                    monitors: frames
                        .into_iter()
                        .map(|(monitor, pixels)| {
                            let width = monitor.physical_width;
                            let height = monitor.physical_height;
                            (monitor, pixels, width, height)
                        })
                        .collect(),
                },
                Err(message) => Outcome::SnapshotFailed { message },
            },
            Request::CaptureNew {
                source,
                x,
                y,
                width,
                height,
                pixels,
            } => self.capture_new(port, source, x, y, width, height, pixels),
            Request::CaptureRefresh { number, hide } => self.capture_refresh(port, number, &hide),
            Request::WindowMoved { number, x, y, width, height } => {
                let Some(capture) = self.captures.iter_mut().find(|capture| capture.number == number) else {
                    return Outcome::Ignored;
                };
                capture.view.x = x;
                capture.view.y = y;
                capture.view.width = width;
                capture.view.height = height;
                Outcome::ViewUpdated
            }
            Request::ShownToggled { number } => {
                let Some(capture) = self.captures.iter_mut().find(|capture| capture.number == number) else {
                    return Outcome::Ignored;
                };
                if capture.previous.is_some() {
                    capture.view.shown = match capture.view.shown {
                        ShownFrame::Current => ShownFrame::Previous,
                        ShownFrame::Previous => ShownFrame::Current,
                    };
                }
                Outcome::ViewUpdated
            }
            Request::Copy { number } => {
                let Some(frame) = self.shown(number) else {
                    return Outcome::CopyFailed { message: self.missing(number) };
                };
                match port.copy_image(&frame.pixels, frame.width, frame.height) {
                    Ok(()) => Outcome::CopySucceeded,
                    Err(err) => Outcome::CopyFailed { message: self.io_failed(language::COPY_FAILED, number, &err) },
                }
            }
            Request::Save { number, path } => {
                let Some(frame) = self.shown(number) else {
                    return Outcome::SaveFailed { message: self.missing(number) };
                };
                match port.write_png(&path, &frame.pixels, frame.width, frame.height) {
                    Ok(()) => Outcome::SaveSucceeded,
                    Err(err) => Outcome::SaveFailed { message: self.io_failed(language::SAVE_FAILED, number, &err) },
                }
            }
            Request::Close { number } => {
                let Some(index) = self.captures.iter().position(|capture| capture.number == number) else {
                    return Outcome::Ignored;
                };
                self.captures.remove(index);
                Outcome::Closed
            }
            Request::LanguageChosen { language } => {
                self.language = language;
                match port.write_language(language) {
                    Ok(()) => Outcome::LanguageSaved { message: None },
                    Err(err) => Outcome::LanguageSaved {
                        message: Some(language::render(
                            self.language,
                            language::LANGUAGE_SAVE_FAILED,
                            &[("err", &err)],
                        )),
                    },
                }
            }
        }
    }

    fn capture_new(
        &mut self,
        port: &P,
        source: CaptureSource,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Outcome {
        let CaptureSource::Region { rect, .. } = &source else {
            return Outcome::CaptureFailed { message: self.plain(language::SOURCE_UNSUPPORTED) };
        };
        if self.captures.len() >= self.policy.max_captures {
            return Outcome::CaptureFailed { message: self.plain(language::WINDOW_LIMIT) };
        }
        let frame_width = rect.width;
        let frame_height = rect.height;
        if !frame_accepted(frame_width, frame_height, pixels.len(), &self.policy) {
            return Outcome::CaptureFailed { message: self.plain(language::FRAME_TOO_BIG) };
        }
        let used = self.used_bytes();
        if used.checked_add(pixels.len()).is_none_or(|total| total > self.policy.byte_budget) {
            return Outcome::CaptureFailed {
                message: language::render(
                    self.language,
                    language::FRAME_BUDGET,
                    &[
                        ("w", &frame_width.to_string()),
                        ("h", &frame_height.to_string()),
                        ("bytes", &pixels.len().to_string()),
                        ("used", &used.to_string()),
                    ],
                ),
            };
        }
        let number = self.next_number;
        self.next_number += 1;
        self.captures.push(HeldCapture {
            number,
            source: source.clone(),
            current: CaptureFrame {
                pixels: Arc::from(pixels),
                width: frame_width,
                height: frame_height,
                captured_at: port.now(),
                source,
            },
            previous: None,
            view: CaptureView {
                x,
                y,
                width,
                height,
                opacity: 255,
                shown: ShownFrame::Current,
            },
        });
        Outcome::CaptureSucceeded { number }
    }

    fn capture_refresh(&mut self, port: &mut P, number: u64, hide: &[P::Window]) -> Outcome {
        let Some(index) = self.captures.iter().position(|capture| capture.number == number) else {
            return Outcome::CaptureFailed { message: self.missing(number) };
        };
        let CaptureSource::Region { rect, fingerprint } = self.captures[index].source.clone() else {
            return Outcome::CaptureFailed { message: self.plain(language::SOURCE_UNSUPPORTED) };
        };
        let monitors = match port.monitors() {
            Ok(monitors) => monitors,
            Err(_) => {
                return Outcome::SourceUnavailable { message: self.plain(language::MONITOR_MISSING) };
            }
        };
        match refresh_gate(&fingerprint, &monitors) {
            Ok(()) => {}
            Err(RefreshBlock::MonitorMissing) => {
                return Outcome::SourceUnavailable { message: self.plain(language::MONITOR_MISSING) };
            }
            Err(RefreshBlock::GeometryChanged) => {
                return Outcome::CaptureFailed { message: self.plain(language::GEOMETRY_CHANGED) };
            }
        }
        let (width, height, pixels) = match port.capture_region(&fingerprint.id, &rect, hide) {
            Ok(captured) => captured,
            Err(message) => return Outcome::CaptureFailed { message },
        };
        if width != rect.width || height != rect.height {
            return Outcome::CaptureFailed {
                message: language::render(
                    self.language,
                    language::REFRESH_SIZE,
                    &[
                        ("number", &number.to_string()),
                        ("w", &width.to_string()),
                        ("h", &height.to_string()),
                        ("rw", &rect.width.to_string()),
                        ("rh", &rect.height.to_string()),
                        ("id", &fingerprint.id),
                    ],
                ),
            };
        }
        if !frame_accepted(width, height, pixels.len(), &self.policy) {
            return Outcome::CaptureFailed { message: self.plain(language::FRAME_TOO_BIG) };
        }
        let used = self.used_bytes();
        let previous_bytes = self.captures[index]
            .previous
            .as_ref()
            .map(|frame| frame.pixels.len())
            .unwrap_or(0);
        let without_previous = used.saturating_sub(previous_bytes);
        if without_previous
            .checked_add(pixels.len())
            .is_none_or(|total| total > self.policy.byte_budget)
        {
            return Outcome::CaptureFailed {
                message: language::render(
                    self.language,
                    language::REFRESH_BUDGET,
                    &[
                        ("number", &number.to_string()),
                        ("w", &width.to_string()),
                        ("h", &height.to_string()),
                        ("bytes", &pixels.len().to_string()),
                        ("used", &used.to_string()),
                        ("id", &fingerprint.id),
                    ],
                ),
            };
        }
        let captured_at = port.now();
        let source = self.captures[index].source.clone();
        let previous = self.captures[index].current.clone();
        self.captures[index].previous = Some(previous);
        self.captures[index].current = CaptureFrame {
            pixels: Arc::from(pixels),
            width,
            height,
            captured_at,
            source,
        };
        self.captures[index].view.shown = ShownFrame::Current;
        Outcome::CaptureSucceeded { number }
    }

    fn shown(&self, number: u64) -> Option<&CaptureFrame> {
        let capture = self.captures.iter().find(|capture| capture.number == number)?;
        Some(match capture.view.shown {
            ShownFrame::Previous => capture.previous.as_ref().unwrap_or(&capture.current),
            ShownFrame::Current => &capture.current,
        })
    }

    fn used_bytes(&self) -> usize {
        self.captures.iter().fold(0usize, |total, capture| {
            let with_current = total.saturating_add(capture.current.pixels.len());
            with_current.saturating_add(
                capture
                    .previous
                    .as_ref()
                    .map(|frame| frame.pixels.len())
                    .unwrap_or(0),
            )
        })
    }

    fn plain(&self, phrase: language::Phrase) -> String {
        language::text(self.language, phrase).to_string()
    }

    fn missing(&self, number: u64) -> String {
        language::render(
            self.language,
            language::CAPTURE_NOT_FOUND,
            &[("number", &number.to_string())],
        )
    }

    fn io_failed(&self, phrase: language::Phrase, number: u64, err: &str) -> String {
        language::render(
            self.language,
            phrase,
            &[("number", &number.to_string()), ("err", err)],
        )
    }
}
