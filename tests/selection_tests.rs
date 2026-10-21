use screenpatch::core::refresh::image_frame;
use screenpatch::core::selection::{
    clamp_physical_point, compose_overlay, crop_frame, label_box, logical_cursor_to_physical,
    normalize_drag, selection_accepted,
};
use screenpatch::core::geometry::CaptureRect;

fn solid(width: u32, height: u32, pixel: [u8; 4]) -> screenpatch::core::refresh::ImageFrame {
    let mut pixels = Vec::new();
    for _ in 0..width * height {
        pixels.extend_from_slice(&pixel);
    }
    image_frame(width, height, pixels).unwrap()
}

fn rgba_at(frame: &screenpatch::core::refresh::ImageFrame, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * frame.width + x) * 4) as usize;
    frame.pixels[index..index + 4].try_into().unwrap()
}

#[test]
fn logical_cursor_scales_and_clamps() {
    assert_eq!(
        logical_cursor_to_physical(10.0, 4.0, 1.5, 1920, 1080).unwrap(),
        (15, 6)
    );
    assert_eq!(
        logical_cursor_to_physical(1.0, 0.0, 1.5, 100, 100).unwrap(),
        (2, 0)
    );
    assert!(logical_cursor_to_physical(1.0, 1.0, 0.0, 100, 100).is_none());
}

#[test]
fn negative_point_clamps_to_monitor_edges() {
    assert_eq!(clamp_physical_point(-5, 5000, 1920, 1080), (0, 1080));
}

#[test]
fn drag_normalizes_in_four_directions() {
    let expected = CaptureRect {
        x: 40,
        y: 80,
        width: 60,
        height: 120,
    };
    let pairs = [
        ((100, 80), (40, 200)),
        ((40, 200), (100, 80)),
        ((40, 80), (100, 200)),
        ((100, 200), (40, 80)),
    ];
    for ((ax, ay), (cx, cy)) in pairs {
        assert_eq!(normalize_drag(ax, ay, cx, cy), expected);
    }
}

#[test]
fn selection_requires_both_sides_at_least_16() {
    assert!(selection_accepted(CaptureRect {
        x: 0,
        y: 0,
        width: 16,
        height: 16
    }));
    assert!(!selection_accepted(CaptureRect {
        x: 0,
        y: 0,
        width: 16,
        height: 15
    }));
    assert!(!selection_accepted(CaptureRect {
        x: 0,
        y: 0,
        width: 15,
        height: 16
    }));
    assert!(!selection_accepted(CaptureRect {
        x: 0,
        y: 0,
        width: 0,
        height: 0
    }));
}

#[test]
fn crop_copies_pixels_and_rejects_a_rect_outside_the_frame() {
    let frame = image_frame(
        2,
        2,
        vec![1, 0, 0, 9, 2, 0, 0, 9, 3, 0, 0, 9, 4, 0, 0, 9],
    )
    .unwrap();
    let cropped = crop_frame(
        &frame,
        CaptureRect {
            x: 1,
            y: 0,
            width: 1,
            height: 2,
        },
    )
    .unwrap();
    assert_eq!(cropped.width, 1);
    assert_eq!(cropped.height, 2);
    assert_eq!(cropped.pixels.as_ref(), &[2, 0, 0, 9, 4, 0, 0, 9]);
    assert!(crop_frame(
        &frame,
        CaptureRect {
            x: 1,
            y: 0,
            width: 2,
            height: 1
        },
    )
    .is_none());
}

#[test]
fn overlay_dims_outside_keeps_inside_and_draws_the_border() {
    let source = solid(80, 80, [255, 1, 2, 200]);
    let composed = compose_overlay(
        &source,
        Some(CaptureRect {
            x: 2,
            y: 2,
            width: 2,
            height: 2,
        }),
    );
    assert_eq!(rgba_at(&composed, 0, 0), [127, 0, 1, 255]);
    assert_eq!(rgba_at(&composed, 2, 2), [255, 1, 2, 200]);
    assert_eq!(rgba_at(&composed, 2, 1), [255, 255, 255, 255]);
    assert_eq!(rgba_at(&composed, 70, 70), [127, 0, 1, 255]);
}

#[test]
fn border_is_clipped_at_the_monitor_edge() {
    let source = solid(80, 80, [10, 10, 10, 10]);
    let composed = compose_overlay(
        &source,
        Some(CaptureRect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        }),
    );
    assert_eq!(rgba_at(&composed, 0, 0), [10, 10, 10, 10]);
    assert_eq!(rgba_at(&composed, 0, 2), [255, 255, 255, 255]);
    assert_eq!(rgba_at(&composed, 3, 3), [5, 5, 5, 255]);
}

#[test]
fn label_box_uses_above_then_below_then_inside() {
    let text = "60 × 120";
    let rect = CaptureRect {
        x: 40,
        y: 80,
        width: 60,
        height: 120,
    };
    assert_eq!(label_box(text, rect, 1920, 1080), (40, 54, 98, 18));

    let near_top = CaptureRect {
        x: 40,
        y: 10,
        width: 60,
        height: 120,
    };
    assert_eq!(label_box(text, near_top, 1920, 1080), (40, 138, 98, 18));

    let tight = CaptureRect {
        x: 0,
        y: 0,
        width: 60,
        height: 30,
    };
    assert_eq!(label_box(text, tight, 100, 40), (0, 0, 98, 18));
}
