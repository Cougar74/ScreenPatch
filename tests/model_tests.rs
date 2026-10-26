use screenpatch::core::geometry::{CaptureRect, MAX_FRAME_BYTES};
use screenpatch::core::language::{text, Language, CAPTURE_NOT_FOUND, SOURCE_UNSUPPORTED};
use screenpatch::core::model::{default_policy, frame_accepted};
use screenpatch::core::refresh::Fingerprint;

#[test]
fn new_phrases_match_the_spec() {
    assert_eq!(text(Language::Ru, SOURCE_UNSUPPORTED), "источник снимка не поддерживается");
    assert_eq!(text(Language::En, SOURCE_UNSUPPORTED), "the capture source is not supported");
    assert_eq!(text(Language::Ru, CAPTURE_NOT_FOUND), "снимок {number} не найден");
    assert_eq!(text(Language::En, CAPTURE_NOT_FOUND), "capture {number} was not found");
}

#[test]
fn default_policy_matches_the_current_limits() {
    let policy = default_policy();
    assert_eq!(policy.max_captures, 10);
    assert_eq!(policy.byte_budget, 512 * 1024 * 1024);
    assert_eq!(policy.max_frame_bytes, MAX_FRAME_BYTES);
}

#[test]
fn frame_accepted_rejects_a_short_buffer_and_a_frame_over_64_mib() {
    let policy = default_policy();
    assert!(frame_accepted(2, 2, 16, &policy));
    assert!(!frame_accepted(2, 2, 15, &policy));
    assert!(!frame_accepted(u32::MAX, u32::MAX, usize::MAX, &policy));
    let _region = CaptureRect { x: 0, y: 0, width: 1, height: 1 };
    let _print = Fingerprint {
        id: "m".to_string(),
        origin_x: 0,
        origin_y: 0,
        physical_width: 1,
        physical_height: 1,
        scale: 1.0,
    };
}
