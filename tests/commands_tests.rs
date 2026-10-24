use screenpatch::core::commands::{
    button_rects, client_hit, draw_buttons, strip_press_target, ButtonIcon, PanelAction,
};
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
fn visible_buttons_hit_their_centers_and_miss_the_gaps() {
    assert_eq!(client_hit(96, 23, 400, 300, true), PanelAction::Refresh);
    assert_eq!(client_hit(148, 23, 400, 300, true), PanelAction::BeforeAfter);
    assert_eq!(client_hit(200, 23, 400, 300, true), PanelAction::Copy);
    assert_eq!(client_hit(252, 23, 400, 300, true), PanelAction::Save);
    assert_eq!(client_hit(304, 23, 400, 300, true), PanelAction::Close);
    assert_eq!(client_hit(122, 23, 400, 300, true), PanelAction::Drag);
    assert_eq!(client_hit(96, 50, 400, 300, true), PanelAction::Drag);
    assert_eq!(client_hit(74, 8, 400, 300, true), PanelAction::Drag);
}

#[test]
fn resize_corner_wins_over_a_button() {
    assert_eq!(client_hit(130, 30, 140, 40, true), PanelAction::Resize);
}

#[test]
fn narrow_row_keeps_a_28_pixel_button_and_clips_the_group() {
    let rects = button_rects(140, 80);
    assert!(rects.iter().all(|rect| rect.width == 28 && rect.height == 30));
    assert_eq!(rects[1].x - (rects[0].x + rects[0].width as i32), 2);
    assert_eq!(rects[0].x, -4);
}

#[test]
fn wide_row_uses_44_by_30_buttons_with_an_8_pixel_gap() {
    let rects = button_rects(400, 300);
    assert!(rects.iter().all(|rect| rect.width == 44 && rect.height == 30 && rect.y == 8));
    assert_eq!(rects[0].x, 74);
    assert_eq!(rects[1].x - (rects[0].x + 44), 8);
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

fn blend(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let alpha = u32::from(src[3]);
    let inverse = 255 - alpha;
    [
        ((u32::from(src[0]) * alpha + u32::from(dst[0]) * inverse) / 255) as u8,
        ((u32::from(src[1]) * alpha + u32::from(dst[1]) * inverse) / 255) as u8,
        ((u32::from(src[2]) * alpha + u32::from(dst[2]) * inverse) / 255) as u8,
        255,
    ]
}

fn frame_pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * width + x) * 4) as usize;
    pixels[index..index + 4].try_into().unwrap()
}

#[test]
fn buttons_blend_over_the_screenshot_and_a_missing_icon_stays_gray() {
    let mut pixels = vec![0u8; 400 * 300 * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[10, 20, 30, 255]);
    }
    let mut icon = vec![0u8; 16 * 16 * 4];
    icon[8 * 16 * 4 + 8 * 4..8 * 16 * 4 + 8 * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
    let glyph = ButtonIcon {
        width: 16,
        height: 16,
        pixels: icon,
    };
    let mut icons = [None, None, None, None, None];
    icons[0] = Some(glyph);
    draw_buttons(
        &mut pixels,
        400,
        300,
        1.0,
        Some(PanelAction::BeforeAfter),
        &icons,
    );
    let background = [10, 20, 30, 255];
    assert_eq!(
        frame_pixel(&pixels, 400, 90, 10),
        blend(background, [32, 32, 32, 170])
    );
    assert_eq!(
        frame_pixel(&pixels, 400, 148, 10),
        blend(background, [14, 14, 14, 220])
    );
    assert_eq!(frame_pixel(&pixels, 400, 96, 23), [255, 255, 255, 255]);
    assert_eq!(
        frame_pixel(&pixels, 400, 200, 23),
        blend(background, [32, 32, 32, 170])
    );
    assert_eq!(frame_pixel(&pixels, 400, 0, 0), background);
}

#[test]
fn decode_button_icon_rejects_bytes_that_are_not_an_image() {
    assert!(screenpatch::core::commands::decode_button_icon(&[]).is_none());
    assert!(screenpatch::core::commands::decode_button_icon(b"not a png").is_none());
}

#[test]
fn button_icon_files_are_transparent_white_glyphs() {
    for name in ["refresh.png", "before-after.png", "copy.png", "save.png", "close.png"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/icons")
            .join(name);
        let bytes = std::fs::read(&path).unwrap();
        let icon = screenpatch::core::commands::decode_button_icon(&bytes).unwrap();
        assert!(icon.width >= 64 && icon.width == icon.height);
        assert!(icon
            .pixels
            .chunks_exact(4)
            .any(|pixel| pixel == [255, 255, 255, 255]));
        assert!(icon.pixels.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(icon
            .pixels
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0 && pixel[3] < 255));
    }
}
