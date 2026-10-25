use std::time::{Duration, Instant};

use crate::core::commands::PanelAction;
use crate::core::language::{
    Phrase, STRIP_BEFORE_AFTER, STRIP_CLOSE, STRIP_COPY, STRIP_REFRESH, STRIP_SAVE,
};

pub const TOOLTIP_DELAY: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooltipHover<Id> {
    pub window: Id,
    pub action: PanelAction,
    pub started: Instant,
    pub shown: bool,
}

pub fn note_hover<Id: Copy + Eq>(
    current: Option<TooltipHover<Id>>,
    window: Id,
    action: PanelAction,
    now: Instant,
) -> TooltipHover<Id> {
    if let Some(current) = current {
        if current.window == window && current.action == action {
            return current;
        }
    }
    TooltipHover {
        window,
        action,
        started: now,
        shown: false,
    }
}

pub fn tooltip_due<Id>(hover: &TooltipHover<Id>, now: Instant) -> bool {
    !hover.shown && now.saturating_duration_since(hover.started) >= TOOLTIP_DELAY
}

pub fn button_tip(action: PanelAction) -> Option<Phrase> {
    match action {
        PanelAction::Refresh => Some(STRIP_REFRESH),
        PanelAction::BeforeAfter => Some(STRIP_BEFORE_AFTER),
        PanelAction::Copy => Some(STRIP_COPY),
        PanelAction::Save => Some(STRIP_SAVE),
        PanelAction::Close => Some(STRIP_CLOSE),
        PanelAction::Drag | PanelAction::Resize => None,
    }
}

pub fn tooltip_client_size(text_width: i32, text_height: i32, pad_x: i32, pad_y: i32) -> (i32, i32) {
    (
        text_width.saturating_add(pad_x.saturating_mul(2)).max(1),
        text_height.saturating_add(pad_y.saturating_mul(2)).max(1),
    )
}

pub fn tooltip_origin(
    cursor_x: i32,
    cursor_y: i32,
    tip_width: i32,
    tip_height: i32,
    work: Option<(i32, i32, u32, u32)>,
) -> (i32, i32) {
    let preferred = (cursor_x.saturating_add(16), cursor_y.saturating_add(20));
    let Some((work_x, work_y, work_width, work_height)) = work else {
        return preferred;
    };
    let tip_width = tip_width.max(0);
    let tip_height = tip_height.max(0);
    let work_right = work_x.saturating_add(i32::try_from(work_width).unwrap_or(i32::MAX));
    let work_bottom = work_y.saturating_add(i32::try_from(work_height).unwrap_or(i32::MAX));
    let mut x = preferred.0;
    let mut y = preferred.1;
    if x.saturating_add(tip_width) > work_right {
        x = cursor_x.saturating_sub(16).saturating_sub(tip_width);
    }
    if y.saturating_add(tip_height) > work_bottom {
        y = cursor_y.saturating_sub(8).saturating_sub(tip_height);
    }
    (x, y) = clamp_tooltip(x, y, tip_width, tip_height, work_x, work_y, work_right, work_bottom);
    if tooltip_contains(x, y, tip_width, tip_height, cursor_x, cursor_y) {
        let room_right = work_right.saturating_sub(cursor_x);
        let room_left = cursor_x.saturating_sub(work_x);
        let room_below = work_bottom.saturating_sub(cursor_y);
        let room_above = cursor_y.saturating_sub(work_y);
        x = if room_right >= room_left {
            cursor_x.saturating_add(16)
        } else {
            cursor_x.saturating_sub(16).saturating_sub(tip_width)
        };
        y = if room_below >= room_above {
            cursor_y.saturating_add(20)
        } else {
            cursor_y.saturating_sub(8).saturating_sub(tip_height)
        };
        (x, y) = clamp_tooltip(x, y, tip_width, tip_height, work_x, work_y, work_right, work_bottom);
    }
    (x, y)
}

fn clamp_tooltip(
    mut x: i32,
    mut y: i32,
    tip_width: i32,
    tip_height: i32,
    work_x: i32,
    work_y: i32,
    work_right: i32,
    work_bottom: i32,
) -> (i32, i32) {
    if x < work_x {
        x = work_x;
    }
    if y < work_y {
        y = work_y;
    }
    if x.saturating_add(tip_width) > work_right {
        x = work_right.saturating_sub(tip_width);
    }
    if y.saturating_add(tip_height) > work_bottom {
        y = work_bottom.saturating_sub(tip_height);
    }
    (x, y)
}

fn tooltip_contains(left: i32, top: i32, tip_width: i32, tip_height: i32, x: i32, y: i32) -> bool {
    x >= left && y >= top && x < left.saturating_add(tip_width) && y < top.saturating_add(tip_height)
}
