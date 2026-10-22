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

pub const STRIP_HEIGHT: i32 = 48;
const ROW_HEIGHT: i32 = 16;
const RESIZE_GRIP: i32 = 16;

pub fn client_hit(x: i32, y: i32, width: u32, height: u32, strip_visible: bool) -> PanelAction {
    if strip_visible {
        return panel_hit(x, y, width, height);
    }
    if width == 0 || height == 0 || x < 0 || y < 0 {
        return PanelAction::Drag;
    }
    let Ok(width) = i32::try_from(width) else {
        return PanelAction::Drag;
    };
    let Ok(height) = i32::try_from(height) else {
        return PanelAction::Drag;
    };
    if x >= width - RESIZE_GRIP && y >= height - RESIZE_GRIP && x < width && y < height {
        return PanelAction::Resize;
    }
    PanelAction::Drag
}

pub fn panel_hit(x: i32, y: i32, width: u32, height: u32) -> PanelAction {
    if width == 0 || height == 0 || x < 0 || y < 0 {
        return PanelAction::Drag;
    }
    let Ok(width) = i32::try_from(width) else {
        return PanelAction::Drag;
    };
    let Ok(height) = i32::try_from(height) else {
        return PanelAction::Drag;
    };
    if x >= width || y >= height {
        return PanelAction::Drag;
    }
    if x >= width - RESIZE_GRIP && y >= height - RESIZE_GRIP {
        return PanelAction::Resize;
    }
    if y >= STRIP_HEIGHT {
        return PanelAction::Drag;
    }
    let mid = width / 2;
    match y / ROW_HEIGHT {
        0 if x < mid => PanelAction::Refresh,
        0 => PanelAction::BeforeAfter,
        1 if x < mid => PanelAction::Copy,
        1 => PanelAction::Save,
        _ => PanelAction::Close,
    }
}
