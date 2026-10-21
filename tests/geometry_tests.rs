use screenpatch::core::geometry::{
    capture_rect_for_monitor, frame_byte_len, global_to_monitor_local, logical_window_size,
    place_window, select_monitor_pair, CaptureRect, LogicalRect, MonitorCandidate, PatchGeometry,
    WorkArea, MAX_FRAME_BYTES,
};

#[test]
fn global_point_on_monitor_with_negative_origin() {
    assert_eq!(global_to_monitor_local(0, 0, -1920, -1080), (1920, 1080));
    assert_eq!(global_to_monitor_local(100, 50, 0, 0), (100, 50));
    assert_eq!(global_to_monitor_local(80, 200, 100, 200), (-20, 0));
}

#[test]
fn logical_window_size_matches_spec_examples() {
    assert_eq!(logical_window_size(640, 400, 1.0).unwrap(), (640, 400));
    assert_eq!(logical_window_size(640, 400, 1.5).unwrap(), (427, 267));
    assert_eq!(logical_window_size(640, 400, 2.0).unwrap(), (320, 200));
}

#[test]
fn logical_window_size_rejects_non_positive_scale() {
    assert!(logical_window_size(640, 400, 0.0).is_none());
    assert!(logical_window_size(640, 400, -1.0).is_none());
}

#[test]
fn default_capture_rect_is_64_64_640x400() {
    let rect = capture_rect_for_monitor(2560, 1440).unwrap();
    assert_eq!(
        rect,
        CaptureRect {
            x: 64,
            y: 64,
            width: 640,
            height: 400
        }
    );
}

#[test]
fn capture_rect_shrinks_to_keep_64px_margin() {
    let rect = capture_rect_for_monitor(700, 500).unwrap();
    assert_eq!(
        rect,
        CaptureRect {
            x: 64,
            y: 64,
            width: 572,
            height: 372
        }
    );
}

#[test]
fn capture_rect_rejects_result_below_16() {
    assert!(capture_rect_for_monitor(140, 140).is_none());
    assert!(capture_rect_for_monitor(100, 800).is_none());
}

#[test]
fn frame_byte_len_rejects_overflow_and_over_64_mib() {
    assert_eq!(frame_byte_len(640, 400), Some(640 * 400 * 4));
    assert_eq!(frame_byte_len(4096, 4096), Some(MAX_FRAME_BYTES));
    assert!(frame_byte_len(4097, 4096).is_none());
    assert!(frame_byte_len(u32::MAX, u32::MAX).is_none());
}

#[test]
fn moving_window_does_not_change_capture_rect() {
    let mut patch = PatchGeometry {
        region: CaptureRect {
            x: 64,
            y: 64,
            width: 640,
            height: 400,
        },
        window_x: 10,
        window_y: 20,
    };
    patch.move_to(800, -40);
    assert_eq!(patch.window_x, 800);
    assert_eq!(patch.window_y, -40);
    assert_eq!(
        patch.region,
        CaptureRect {
            x: 64,
            y: 64,
            width: 640,
            height: 400
        }
    );
}

#[test]
fn place_window_prefers_the_right_side() {
    let pos = place_window(
        LogicalRect {
            x: 43,
            y: 43,
            width: 427,
            height: 267,
        },
        427,
        267,
        WorkArea {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
    );
    assert_eq!(pos, (43 + 427 + 16, 43));
}

#[test]
fn place_window_uses_below_when_right_does_not_fit() {
    let pos = place_window(
        LogicalRect {
            x: 10,
            y: 10,
            width: 100,
            height: 40,
        },
        80,
        30,
        WorkArea {
            x: 0,
            y: 0,
            width: 180,
            height: 200,
        },
    );
    assert_eq!(pos, (10, 10 + 40 + 16));
}

#[test]
fn place_window_uses_farthest_corner_when_sides_do_not_fit() {
    let pos = place_window(
        LogicalRect {
            x: 10,
            y: 10,
            width: 100,
            height: 100,
        },
        80,
        80,
        WorkArea {
            x: 0,
            y: 0,
            width: 150,
            height: 150,
        },
    );
    assert_eq!(pos, (70, 70));
}

#[test]
fn select_monitor_pair_picks_earliest_different_scales() {
    let monitors = [
        MonitorCandidate {
            scale: 1.0,
            physical_width: 1920,
            physical_height: 1080,
        },
        MonitorCandidate {
            scale: 1.0,
            physical_width: 1920,
            physical_height: 1080,
        },
        MonitorCandidate {
            scale: 1.5,
            physical_width: 2560,
            physical_height: 1440,
        },
    ];
    assert_eq!(select_monitor_pair(&monitors), Some((0, 2)));
}

#[test]
fn select_monitor_pair_skips_monitors_that_cannot_fit_a_rect() {
    let monitors = [
        MonitorCandidate {
            scale: 2.0,
            physical_width: 100,
            physical_height: 100,
        },
        MonitorCandidate {
            scale: 1.0,
            physical_width: 1920,
            physical_height: 1080,
        },
        MonitorCandidate {
            scale: 2.0,
            physical_width: 1920,
            physical_height: 1080,
        },
    ];
    assert_eq!(select_monitor_pair(&monitors), Some((1, 2)));
}

#[test]
fn select_monitor_pair_returns_none_when_scales_match() {
    let monitors = [
        MonitorCandidate {
            scale: 1.5,
            physical_width: 1920,
            physical_height: 1080,
        },
        MonitorCandidate {
            scale: 1.5,
            physical_width: 2560,
            physical_height: 1440,
        },
    ];
    assert!(select_monitor_pair(&monitors).is_none());
}
