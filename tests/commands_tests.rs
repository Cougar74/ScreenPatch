use screenpatch::core::commands::{panel_hit, strip_press_target, PanelAction};
use screenpatch::core::geometry::CaptureRect;
use screenpatch::core::patches::PatchManager;
use screenpatch::core::refresh::image_frame;
use screenpatch::platform::draw_strip;

fn frame(width: u32, height: u32, fill: u8) -> screenpatch::core::refresh::ImageFrame {
    let len = (width * height * 4) as usize;
    image_frame(width, height, vec![fill; len]).unwrap()
}

fn region() -> CaptureRect {
    CaptureRect {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    }
}

#[test]
fn panel_points_match_the_120x80_grid() {
    assert_eq!(panel_hit(10, 4, 120, 80), PanelAction::Refresh);
    assert_eq!(panel_hit(70, 4, 120, 80), PanelAction::BeforeAfter);
    assert_eq!(panel_hit(10, 20, 120, 80), PanelAction::Copy);
    assert_eq!(panel_hit(70, 20, 120, 80), PanelAction::Save);
    assert_eq!(panel_hit(10, 40, 120, 80), PanelAction::Close);
    assert_eq!(panel_hit(10, 48, 120, 80), PanelAction::Drag);
    assert_eq!(panel_hit(104, 64, 120, 80), PanelAction::Resize);
}

#[test]
fn resize_corner_wins_when_it_overlaps_the_strip() {
    assert_eq!(panel_hit(104, 30, 120, 40), PanelAction::Resize);
}

#[test]
fn before_after_without_a_previous_frame_keeps_the_current_view() {
    let mut manager = PatchManager::new();
    let original = frame(2, 2, 1);
    let number = manager.add(region(), original.clone()).unwrap();

    assert!(!manager.toggle_shown(number));
    assert_eq!(manager.shown_frame(number).unwrap(), &original);
}

#[test]
fn before_after_flips_and_refresh_returns_to_the_current_frame() {
    let mut manager = PatchManager::new();
    let original = frame(2, 2, 1);
    let number = manager.add(region(), original.clone()).unwrap();
    let updated = frame(2, 2, 9);
    manager.commit_refresh(number, updated.clone()).unwrap();

    assert!(manager.toggle_shown(number));
    assert_eq!(manager.shown_frame(number).unwrap(), &original);
    let stored = manager.get(number).unwrap();
    assert_eq!(stored.region, region());
    assert_eq!(stored.frame, updated);
    assert_eq!(stored.previous.as_ref(), Some(&original));

    assert!(manager.toggle_shown(number));
    assert_eq!(manager.shown_frame(number).unwrap(), &updated);

    let newest = frame(2, 2, 3);
    manager.commit_refresh(number, newest.clone()).unwrap();
    assert_eq!(manager.shown_frame(number).unwrap(), &newest);
}

#[test]
fn export_uses_the_shown_frame_at_its_original_size() {
    let mut manager = PatchManager::new();
    let number = manager.add(region(), frame(2, 2, 1)).unwrap();
    manager.commit_refresh(number, frame(2, 2, 9)).unwrap();
    manager.toggle_shown(number);

    let exported = manager.shown_frame(number).unwrap();
    assert_eq!((exported.width, exported.height), (2, 2));
    assert_eq!(exported.pixels[0], 1);
}

#[test]
fn hiding_windows_keeps_bytes_and_close_all_needs_confirmation() {
    let mut manager = PatchManager::new();
    manager.add(region(), frame(2, 2, 1)).unwrap();
    manager.add(region(), frame(2, 2, 2)).unwrap();
    let used = manager.used_bytes();

    manager.set_windows_visible(false);
    manager.set_windows_visible(true);
    assert_eq!(manager.used_bytes(), used);
    assert_eq!(manager.len(), 2);

    assert!(!manager.close_all(false));
    assert_eq!(manager.len(), 2);
    assert_eq!(manager.used_bytes(), used);

    assert!(manager.close_all(true));
    assert_eq!(manager.len(), 0);
    assert_eq!(manager.used_bytes(), 0);
}

#[test]
fn strip_press_stays_on_the_same_button() {
    assert_eq!(
        strip_press_target(Some(PanelAction::Refresh), PanelAction::Refresh),
        Some(PanelAction::Refresh)
    );
    assert_eq!(
        strip_press_target(Some(PanelAction::Refresh), PanelAction::Copy),
        None
    );
    assert_eq!(
        strip_press_target(Some(PanelAction::Refresh), PanelAction::Refresh),
        Some(PanelAction::Refresh)
    );
    assert_eq!(
        strip_press_target(Some(PanelAction::Refresh), PanelAction::Drag),
        None
    );
    assert_eq!(strip_press_target(Some(PanelAction::Drag), PanelAction::Refresh), None);
    assert_eq!(
        strip_press_target(Some(PanelAction::Resize), PanelAction::Resize),
        None
    );
    assert_eq!(
        strip_press_target(Some(PanelAction::BeforeAfter), PanelAction::BeforeAfter),
        Some(PanelAction::BeforeAfter)
    );
    assert_eq!(strip_press_target(None, PanelAction::Refresh), None);
}

fn strip_pixel(strip: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let index = ((y * width + x) * 4) as usize;
    [strip[index], strip[index + 1], strip[index + 2]]
}

#[test]
fn pressed_strip_cell_is_darker_than_its_neighbor() {
    let labels = ["a", "b", "c", "d", "e"];
    let plain = draw_strip(120, 48, true, labels, None);
    let pressed = draw_strip(120, 48, false, labels, Some(PanelAction::Refresh));
    assert_eq!(strip_pixel(&plain, 120, 2, 2), [32, 32, 32]);
    assert_eq!(strip_pixel(&pressed, 120, 2, 2), [14, 14, 14]);
    assert_eq!(strip_pixel(&pressed, 120, 62, 2), [32, 32, 32]);
    let close = draw_strip(120, 48, true, labels, Some(PanelAction::Close));
    assert_eq!(strip_pixel(&close, 120, 100, 40), [14, 14, 14]);
    assert_eq!(strip_pixel(&close, 120, 2, 2), [32, 32, 32]);
}
