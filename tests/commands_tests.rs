use screenpatch::core::commands::{panel_hit, PanelAction};
use screenpatch::core::geometry::CaptureRect;
use screenpatch::core::patches::PatchManager;
use screenpatch::core::refresh::image_frame;

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
