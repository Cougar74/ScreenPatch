use screenpatch::core::commands::{client_hit, PanelAction};
use screenpatch::core::geometry::{
    logical_client_keeping_physical, one_to_one_physical_size, shift_into_work_area, WorkArea,
};

fn work() -> WorkArea {
    WorkArea { x: 0, y: 0, width: 1920, height: 1080 }
}

#[test]
fn logical_size_keeps_640x400_at_125_percent() {
    assert_eq!(logical_client_keeping_physical(640, 400, 1.25), Some((512, 320)));
}

#[test]
fn logical_size_does_not_raise_a_small_client_to_the_minimum() {
    assert_eq!(logical_client_keeping_physical(50, 40, 1.25), Some((40, 32)));
}

#[test]
fn logical_size_rejects_a_bad_scale_and_a_zero_side() {
    assert_eq!(logical_client_keeping_physical(640, 400, 0.0), None);
    assert_eq!(logical_client_keeping_physical(640, 400, f64::NAN), None);
    assert_eq!(logical_client_keeping_physical(0, 400, 1.0), None);
}

#[test]
fn one_to_one_adds_the_strip_only_when_it_is_visible() {
    assert_eq!(one_to_one_physical_size(640, 400, 1.0, true), Some((640, 448)));
    assert_eq!(one_to_one_physical_size(640, 400, 1.0, false), Some((640, 400)));
    assert_eq!(one_to_one_physical_size(640, 400, 1.25, true), Some((640, 460)));
    assert_eq!(one_to_one_physical_size(10, 10, 1.0, true), Some((10, 58)));
    assert_eq!(one_to_one_physical_size(640, 400, 0.0, true), None);
}

#[test]
fn shift_keeps_a_window_that_is_already_inside() {
    assert_eq!(shift_into_work_area(10, 10, 100, 80, work()), (10, 10));
}

#[test]
fn shift_pulls_an_overflow_back_inside() {
    assert_eq!(shift_into_work_area(-10, 5, 100, 80, work()), (0, 5));
    assert_eq!(shift_into_work_area(1900, 1000, 100, 80, work()), (1820, 1000));
}

#[test]
fn shift_aligns_an_oversized_window_to_the_start_of_that_axis() {
    assert_eq!(shift_into_work_area(100, 10, 2000, 80, work()), (0, 10));
    assert_eq!(shift_into_work_area(10, 100, 100, 2000, work()), (10, 0));
    let negative = WorkArea { x: -1920, y: 0, width: 1920, height: 1080 };
    assert_eq!(shift_into_work_area(0, 10, 2000, 80, negative), (-1920, 10));
}

#[test]
fn hidden_strip_drags_the_old_button_point_and_keeps_the_corner() {
    assert_eq!(client_hit(10, 4, 120, 80, false), PanelAction::Drag);
    assert_eq!(client_hit(104, 64, 120, 80, false), PanelAction::Resize);
    assert_eq!(client_hit(104, 30, 120, 40, false), PanelAction::Resize);
}
