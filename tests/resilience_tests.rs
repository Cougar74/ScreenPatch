use screenpatch::core::refresh::{refresh_gate, Fingerprint, RefreshBlock};
use screenpatch::core::selection::{selection_ready, MonitorReadiness, SelectionAbort};

fn saved() -> Fingerprint {
    Fingerprint {
        id: "mon-a".to_string(),
        origin_x: 1920,
        origin_y: 0,
        physical_width: 1920,
        physical_height: 1080,
        scale: 1.0,
    }
}

fn ready(id: &str) -> MonitorReadiness {
    MonitorReadiness {
        id: id.to_string(),
        scale: 1.25,
        physical_width: 1920,
        physical_height: 1080,
        frame_ready: true,
    }
}

#[test]
fn refresh_gate_accepts_the_same_fingerprint() {
    let monitor = saved();
    assert_eq!(refresh_gate(&saved(), &[monitor]), Ok(()));
}

#[test]
fn refresh_gate_misses_an_empty_list_and_a_different_id() {
    let mut other = saved();
    other.id = "mon-b".to_string();
    assert_eq!(refresh_gate(&saved(), &[]), Err(RefreshBlock::MonitorMissing));
    assert_eq!(
        refresh_gate(&saved(), &[other]),
        Err(RefreshBlock::MonitorMissing)
    );
}

#[test]
fn refresh_gate_rejects_origin_width_and_height_changes() {
    let mut origin = saved();
    origin.origin_x += 1;
    let mut width = saved();
    width.physical_width += 1;
    let mut height = saved();
    height.physical_height += 1;
    assert_eq!(
        refresh_gate(&saved(), &[origin]),
        Err(RefreshBlock::GeometryChanged)
    );
    assert_eq!(
        refresh_gate(&saved(), &[width]),
        Err(RefreshBlock::GeometryChanged)
    );
    assert_eq!(
        refresh_gate(&saved(), &[height]),
        Err(RefreshBlock::GeometryChanged)
    );
}

#[test]
fn refresh_gate_uses_the_existing_scale_tolerance() {
    let mut too_far = saved();
    let mut step = 0.001_f64;
    while (1.0 + step - 1.0).abs() < 0.001 {
        step = f64::from_bits(step.to_bits() + 1);
    }
    too_far.scale = 1.0 + step;
    let mut close = saved();
    close.scale += 0.0009;
    assert_eq!(
        refresh_gate(&saved(), &[too_far]),
        Err(RefreshBlock::GeometryChanged)
    );
    assert_eq!(refresh_gate(&saved(), &[close]), Ok(()));
}

#[test]
fn refresh_gate_does_not_replace_a_missing_id_with_a_similar_monitor() {
    let mut similar = saved();
    similar.id = "mon-b".to_string();
    assert_eq!(
        refresh_gate(&saved(), &[similar]),
        Err(RefreshBlock::MonitorMissing)
    );
}

#[test]
fn refresh_gate_uses_the_first_id_match() {
    let mut changed = saved();
    changed.physical_width += 1;
    let exact = saved();
    assert_eq!(
        refresh_gate(&saved(), &[changed, exact]),
        Err(RefreshBlock::GeometryChanged)
    );
}

#[test]
fn selection_ready_rejects_an_empty_list() {
    assert_eq!(selection_ready(&[]), Err(SelectionAbort::NoMonitors));
}

#[test]
fn selection_ready_accepts_one_and_two_ready_monitors() {
    assert_eq!(selection_ready(&[ready("a")]), Ok(()));
    assert_eq!(selection_ready(&[ready("a"), ready("b")]), Ok(()));
}

#[test]
fn selection_ready_names_the_second_monitor_when_it_fails() {
    let mut second = ready("b");
    second.frame_ready = false;
    assert_eq!(
        selection_ready(&[ready("a"), second]),
        Err(SelectionAbort::CaptureFailed { id: "b".to_string() })
    );
}

#[test]
fn selection_ready_reports_bad_scale_before_size_and_capture() {
    let broken = MonitorReadiness {
        id: "bad".to_string(),
        scale: 0.0,
        physical_width: 0,
        physical_height: 0,
        frame_ready: false,
    };
    let mut non_finite = broken.clone();
    non_finite.id = "nan".to_string();
    non_finite.scale = f64::NAN;
    assert_eq!(
        selection_ready(&[broken]),
        Err(SelectionAbort::BadScale { id: "bad".to_string() })
    );
    assert_eq!(
        selection_ready(&[non_finite]),
        Err(SelectionAbort::BadScale { id: "nan".to_string() })
    );
}

#[test]
fn selection_ready_reports_an_empty_side() {
    let mut width = ready("w");
    width.physical_width = 0;
    let mut height = ready("h");
    height.physical_height = 0;
    assert_eq!(
        selection_ready(&[width]),
        Err(SelectionAbort::EmptySize { id: "w".to_string() })
    );
    assert_eq!(
        selection_ready(&[height]),
        Err(SelectionAbort::EmptySize { id: "h".to_string() })
    );
}

#[test]
fn selection_ready_reports_a_missing_frame() {
    let mut monitor = ready("shot");
    monitor.frame_ready = false;
    assert_eq!(
        selection_ready(&[monitor]),
        Err(SelectionAbort::CaptureFailed {
            id: "shot".to_string()
        })
    );
}

#[test]
fn selection_ready_stops_at_the_first_broken_monitor() {
    let mut first = ready("first");
    first.physical_height = 0;
    assert_eq!(
        selection_ready(&[first, ready("second")]),
        Err(SelectionAbort::EmptySize {
            id: "first".to_string()
        })
    );
}
