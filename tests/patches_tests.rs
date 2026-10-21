use screenpatch::core::geometry::CaptureRect;
use screenpatch::core::patches::{
    fits_budget, initial_logical_size, letterbox, pointer_hit, render_patch, PatchManager,
    PointerHit, PATCH_BYTE_BUDGET,
};
use screenpatch::core::refresh::image_frame;

fn frame(width: u32, height: u32, fill: u8) -> screenpatch::core::refresh::ImageFrame {
    let len = (width * height * 4) as usize;
    image_frame(width, height, vec![fill; len]).unwrap()
}

fn pixel(frame: &screenpatch::core::refresh::ImageFrame, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * frame.width + x) * 4) as usize;
    frame.pixels[index..index + 4].try_into().unwrap()
}

#[test]
fn letterbox_fits_640x400_into_320x400() {
    let placed = letterbox(320, 400, 640, 400).unwrap();
    assert_eq!((placed.x, placed.y, placed.width, placed.height), (0, 100, 320, 200));
}

#[test]
fn render_pads_outside_and_samples_inside_without_filtering() {
    let mut raw = vec![9u8; (640 * 400 * 4) as usize];
    raw[0] = 10;
    raw[1] = 20;
    raw[2] = 30;
    raw[3] = 40;
    let second = 2 * 4;
    raw[second] = 50;
    raw[second + 1] = 60;
    raw[second + 2] = 70;
    raw[second + 3] = 80;
    let source = image_frame(640, 400, raw).unwrap();

    let rendered = render_patch(&source, 320, 400);
    assert_eq!(pixel(&rendered, 0, 0), [24, 24, 24, 255]);
    assert_eq!(pixel(&rendered, 0, 100), [10, 20, 30, 40]);
    assert_eq!(pixel(&rendered, 1, 100), [50, 60, 70, 80]);
}

#[test]
fn initial_window_respects_the_minimum_and_the_one_to_one_size() {
    assert_eq!(initial_logical_size(16, 16, 1.0).unwrap(), (120, 80));
    assert_eq!(initial_logical_size(640, 400, 1.0).unwrap(), (640, 400));
}

#[test]
fn corner_hit_is_resize_and_does_not_change_the_capture() {
    assert_eq!(pointer_hit(104, 64, 120, 80), Some(PointerHit::Resize));
    assert_eq!(pointer_hit(103, 64, 120, 80), Some(PointerHit::Drag));

    let mut manager = PatchManager::new();
    let image = frame(4, 4, 3);
    let region = CaptureRect {
        x: 64,
        y: 64,
        width: 4,
        height: 4,
    };
    let number = manager.add(region, image.clone()).unwrap();
    manager.drag(number, 400, -20);
    manager.resize(number, 300, 200);
    let stored = manager.get(number).unwrap();
    assert_eq!(stored.region, region);
    assert_eq!(stored.frame, image);
    assert_eq!(stored.window_x, 400);
    assert_eq!(stored.window_w, 300);
}

#[test]
fn budget_accepts_the_exact_limit_and_rejects_one_extra_byte() {
    assert!(fits_budget(0, PATCH_BYTE_BUDGET));
    assert!(!fits_budget(1, PATCH_BYTE_BUDGET));
    assert!(fits_budget(PATCH_BYTE_BUDGET - 4, 4));
}

#[test]
fn closing_a_patch_returns_its_bytes_and_keeps_the_other() {
    let mut manager = PatchManager::new();
    let first = frame(2, 2, 1);
    let second = frame(2, 2, 2);
    let first_number = manager
        .add(
            CaptureRect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            first,
        )
        .unwrap();
    let second_number = manager
        .add(
            CaptureRect {
                x: 8,
                y: 8,
                width: 2,
                height: 2,
            },
            second.clone(),
        )
        .unwrap();
    assert_eq!(manager.used_bytes(), 32);
    assert!(manager.close(first_number));
    assert_eq!(manager.used_bytes(), 16);
    assert!(manager.get(first_number).is_none());
    assert_eq!(manager.get(second_number).unwrap().frame, second);
}

#[test]
fn numbers_start_at_one_and_are_not_reused() {
    let mut manager = PatchManager::new();
    let image = frame(2, 2, 1);
    let region = CaptureRect {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };
    let first = manager.add(region, image.clone()).unwrap();
    assert_eq!(first, 1);
    manager.close(first);
    let second = manager.add(region, image).unwrap();
    assert_eq!(second, 2);
}

#[test]
fn eleventh_patch_is_rejected_and_the_first_ten_remain() {
    let mut manager = PatchManager::new();
    let region = CaptureRect {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };
    for index in 0..10 {
        manager.add(region, frame(2, 2, index)).unwrap();
    }
    let error = manager.add(region, frame(2, 2, 99)).unwrap_err();
    assert!(matches!(error, screenpatch::core::patches::AddPatchError::TooMany));
    assert_eq!(manager.len(), 10);
    assert_eq!(manager.get(1).unwrap().frame.pixels[0], 0);
    assert_eq!(manager.get(10).unwrap().frame.pixels[0], 9);
}
