use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use screenpatch::core::refresh::{
    fingerprints_match, image_frame, refresh_current, Fingerprint, ImageFrame,
};

fn frame(width: u32, height: u32, fill: u8) -> ImageFrame {
    let len = (width * height * 4) as usize;
    image_frame(width, height, vec![fill; len]).unwrap()
}

fn fingerprint(scale: f64) -> Fingerprint {
    Fingerprint {
        id: "mon-a".to_string(),
        origin_x: -1920,
        origin_y: 0,
        physical_width: 2560,
        physical_height: 1440,
        scale,
    }
}

#[test]
fn fingerprint_matches_only_when_every_field_matches() {
    let saved = fingerprint(1.5);
    assert!(fingerprints_match(&saved, &fingerprint(1.5)));

    let mut other = fingerprint(1.5);
    other.id = "mon-b".to_string();
    assert!(!fingerprints_match(&saved, &other));

    other = fingerprint(1.5);
    other.origin_x = 0;
    assert!(!fingerprints_match(&saved, &other));

    other = fingerprint(1.5);
    other.physical_height = 1080;
    assert!(!fingerprints_match(&saved, &other));

    other = fingerprint(2.0);
    assert!(!fingerprints_match(&saved, &other));
}

#[test]
fn mismatched_fingerprint_does_not_capture_or_replace_the_frame() {
    let current = frame(2, 2, 1);
    let calls = AtomicUsize::new(0);
    let restored = AtomicBool::new(false);
    let next = refresh_current(
        current.clone(),
        false,
        true,
        || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(frame(2, 2, 9))
        },
        || restored.store(true, Ordering::SeqCst),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(!restored.load(Ordering::SeqCst));
    assert_eq!(next, current);
}

#[test]
fn successful_capture_replaces_the_frame() {
    let current = frame(2, 2, 1);
    let captured = frame(2, 2, 9);
    let next = refresh_current(
        current,
        true,
        false,
        || Ok(captured.clone()),
        || panic!("affinity path must not restore visibility"),
    );
    assert_eq!(next, captured);
}

#[test]
fn capture_error_keeps_the_frame_and_restores_visibility() {
    let current = frame(2, 2, 1);
    let restored = AtomicBool::new(false);
    let next = refresh_current(
        current.clone(),
        true,
        true,
        || Err(()),
        || restored.store(true, Ordering::SeqCst),
    );
    assert!(restored.load(Ordering::SeqCst));
    assert_eq!(next, current);
}

#[test]
fn hide_path_restores_visibility_after_success() {
    let restored = AtomicBool::new(false);
    let _ = refresh_current(
        frame(2, 2, 1),
        true,
        true,
        || Ok(frame(2, 2, 4)),
        || restored.store(true, Ordering::SeqCst),
    );
    assert!(restored.load(Ordering::SeqCst));
}

#[test]
fn image_frame_rejects_a_buffer_over_the_cap() {
    assert!(image_frame(4097, 4096, Vec::new()).is_none());
}
