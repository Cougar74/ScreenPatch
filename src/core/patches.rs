use crate::core::geometry::{logical_window_size, CaptureRect};
use crate::core::refresh::ImageFrame;

pub const PATCH_BYTE_BUDGET: usize = 512 * 1024 * 1024;
pub const MIN_LOGICAL_WIDTH: u32 = 120;
pub const MIN_LOGICAL_HEIGHT: u32 = 80;
pub const RESIZE_GRIP: i32 = 16;
pub const MAX_PATCHES: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Letterbox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerHit {
    Drag,
    Resize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddPatchError {
    TooMany,
    OverBudget { frame_bytes: usize, used_bytes: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitRefreshError {
    OverBudget { frame_bytes: usize, used_bytes: usize },
    SizeMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FramePair {
    pub current: ImageFrame,
    pub previous: Option<ImageFrame>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShownView {
    Current,
    Previous,
}

#[derive(Clone, Debug)]
pub struct HeldPatch {
    pub number: u64,
    pub region: CaptureRect,
    pub frame: ImageFrame,
    pub previous: Option<ImageFrame>,
    pub shown: ShownView,
    pub window_x: i32,
    pub window_y: i32,
    pub window_w: u32,
    pub window_h: u32,
}

#[derive(Debug)]
pub struct PatchManager {
    next_number: u64,
    patches: Vec<HeldPatch>,
    windows_visible: bool,
}

pub fn fits_budget(used: usize, additional: usize) -> bool {
    match used.checked_add(additional) {
        Some(total) => total <= PATCH_BYTE_BUDGET,
        None => false,
    }
}

pub fn commit_refresh(
    pair: &FramePair,
    region_width: u32,
    region_height: u32,
    new_frame: ImageFrame,
    used_bytes: usize,
) -> Result<FramePair, CommitRefreshError> {
    if new_frame.width != region_width || new_frame.height != region_height {
        return Err(CommitRefreshError::SizeMismatch);
    }
    let previous_bytes = pair
        .previous
        .as_ref()
        .map(|frame| frame.pixels.len())
        .unwrap_or(0);
    let without_previous = used_bytes.saturating_sub(previous_bytes);
    if !fits_budget(without_previous, new_frame.pixels.len()) {
        return Err(CommitRefreshError::OverBudget {
            frame_bytes: new_frame.pixels.len(),
            used_bytes,
        });
    }
    Ok(FramePair {
        previous: Some(pair.current.clone()),
        current: new_frame,
    })
}

pub fn letterbox(client_w: u32, client_h: u32, image_w: u32, image_h: u32) -> Option<Letterbox> {
    if client_w == 0 || client_h == 0 || image_w == 0 || image_h == 0 {
        return None;
    }
    let client_width = u64::from(client_w);
    let client_height = u64::from(client_h);
    let image_width = u64::from(image_w);
    let image_height = u64::from(image_h);
    let (width, height) = if client_width * image_height <= client_height * image_width {
        let height = ((client_width * image_height) / image_width) as u32;
        (client_w, height)
    } else {
        let width = ((client_height * image_width) / image_height) as u32;
        (width, client_h)
    };
    if width == 0 || height == 0 {
        return None;
    }
    Some(Letterbox {
        x: (client_w - width) / 2,
        y: (client_h - height) / 2,
        width,
        height,
    })
}

pub fn layout_client(width: u32, height: u32) -> (u32, u32) {
    (width.max(MIN_LOGICAL_WIDTH), height.max(MIN_LOGICAL_HEIGHT))
}

pub fn initial_logical_size(physical_width: u32, physical_height: u32, scale: f64) -> Option<(u32, u32)> {
    let (width, height) = logical_window_size(physical_width, physical_height, scale)?;
    Some((width.max(MIN_LOGICAL_WIDTH), height.max(MIN_LOGICAL_HEIGHT)))
}

pub fn pointer_hit(x: i32, y: i32, width: u32, height: u32) -> Option<PointerHit> {
    if width == 0 || height == 0 || x < 0 || y < 0 {
        return None;
    }
    let width = i32::try_from(width).ok()?;
    let height = i32::try_from(height).ok()?;
    if x >= width || y >= height {
        return None;
    }
    if x >= width - RESIZE_GRIP && y >= height - RESIZE_GRIP {
        Some(PointerHit::Resize)
    } else {
        Some(PointerHit::Drag)
    }
}

pub fn render_patch(frame: &ImageFrame, client_w: u32, client_h: u32) -> ImageFrame {
    let (layout_w, layout_h) = layout_client(client_w, client_h);
    let mut pixels = vec![24u8, 24, 24, 255].repeat((layout_w as usize) * (layout_h as usize));
    if let Some(placed) = letterbox(layout_w, layout_h, frame.width, frame.height) {
        for local_y in 0..placed.height {
            let source_y = ((u64::from(local_y) * u64::from(frame.height)) / u64::from(placed.height)) as u32;
            for local_x in 0..placed.width {
                let source_x =
                    ((u64::from(local_x) * u64::from(frame.width)) / u64::from(placed.width)) as u32;
                let source = ((source_y * frame.width + source_x) * 4) as usize;
                let destination = (((placed.y + local_y) * layout_w + placed.x + local_x) * 4) as usize;
                pixels[destination..destination + 4].copy_from_slice(&frame.pixels[source..source + 4]);
            }
        }
    }
    ImageFrame {
        pixels: std::sync::Arc::from(pixels.into_boxed_slice()),
        width: layout_w,
        height: layout_h,
    }
}

impl PatchManager {
    pub fn new() -> Self {
        Self {
            next_number: 1,
            patches: Vec::new(),
            windows_visible: true,
        }
    }

    pub fn len(&self) -> usize {
        self.patches.len()
    }

    pub fn used_bytes(&self) -> usize {
        self.patches
            .iter()
            .map(|patch| {
                patch.frame.pixels.len()
                    + patch
                        .previous
                        .as_ref()
                        .map(|frame| frame.pixels.len())
                        .unwrap_or(0)
            })
            .sum()
    }

    pub fn get(&self, number: u64) -> Option<&HeldPatch> {
        self.patches.iter().find(|patch| patch.number == number)
    }

    pub fn check(&self, frame: &ImageFrame) -> Result<(), AddPatchError> {
        if self.patches.len() >= MAX_PATCHES {
            return Err(AddPatchError::TooMany);
        }
        let frame_bytes = frame.pixels.len();
        let used_bytes = self.used_bytes();
        if !fits_budget(used_bytes, frame_bytes) {
            return Err(AddPatchError::OverBudget {
                frame_bytes,
                used_bytes,
            });
        }
        Ok(())
    }

    pub fn add(&mut self, region: CaptureRect, frame: ImageFrame) -> Result<u64, AddPatchError> {
        self.check(&frame)?;
        let number = self.next_number;
        self.next_number = self.next_number.saturating_add(1);
        self.patches.push(HeldPatch {
            number,
            region,
            frame,
            previous: None,
            shown: ShownView::Current,
            window_x: 0,
            window_y: 0,
            window_w: 0,
            window_h: 0,
        });
        Ok(number)
    }

    pub fn close(&mut self, number: u64) -> bool {
        let before = self.patches.len();
        self.patches.retain(|patch| patch.number != number);
        self.patches.len() != before
    }

    pub fn drag(&mut self, number: u64, x: i32, y: i32) {
        if let Some(patch) = self.patches.iter_mut().find(|patch| patch.number == number) {
            patch.window_x = x;
            patch.window_y = y;
        }
    }

    pub fn resize(&mut self, number: u64, width: u32, height: u32) {
        if let Some(patch) = self.patches.iter_mut().find(|patch| patch.number == number) {
            patch.window_w = width;
            patch.window_h = height;
        }
    }

    pub fn commit_refresh(
        &mut self,
        number: u64,
        frame: ImageFrame,
    ) -> Result<(), CommitRefreshError> {
        let Some(patch) = self.patches.iter().find(|patch| patch.number == number) else {
            return Err(CommitRefreshError::SizeMismatch);
        };
        let pair = FramePair {
            current: patch.frame.clone(),
            previous: patch.previous.clone(),
        };
        let region_width = patch.region.width;
        let region_height = patch.region.height;
        let next = commit_refresh(&pair, region_width, region_height, frame, self.used_bytes())?;
        let Some(patch) = self.patches.iter_mut().find(|patch| patch.number == number) else {
            return Err(CommitRefreshError::SizeMismatch);
        };
        patch.previous = next.previous;
        patch.frame = next.current;
        patch.shown = ShownView::Current;
        Ok(())
    }

    pub fn toggle_shown(&mut self, number: u64) -> bool {
        let Some(patch) = self.patches.iter_mut().find(|patch| patch.number == number) else {
            return false;
        };
        if patch.previous.is_none() {
            patch.shown = ShownView::Current;
            return false;
        }
        patch.shown = match patch.shown {
            ShownView::Current => ShownView::Previous,
            ShownView::Previous => ShownView::Current,
        };
        true
    }

    pub fn shown_frame(&self, number: u64) -> Option<&ImageFrame> {
        let patch = self.get(number)?;
        match patch.shown {
            ShownView::Current => Some(&patch.frame),
            ShownView::Previous => Some(patch.previous.as_ref().unwrap_or(&patch.frame)),
        }
    }

    pub fn set_windows_visible(&mut self, visible: bool) {
        self.windows_visible = visible;
    }

    #[allow(dead_code)]
    pub fn windows_visible(&self) -> bool {
        self.windows_visible
    }

    pub fn close_all(&mut self, confirmed: bool) -> bool {
        if !confirmed {
            return false;
        }
        self.patches.clear();
        true
    }

    pub fn replace_frame(&mut self, number: u64, frame: ImageFrame) -> bool {
        let Some(patch) = self.patches.iter_mut().find(|patch| patch.number == number) else {
            return false;
        };
        patch.frame = frame;
        true
    }
}

impl Default for PatchManager {
    fn default() -> Self {
        Self::new()
    }
}
