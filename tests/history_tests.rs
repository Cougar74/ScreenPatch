use screenpatch::core::geometry::CaptureRect;
use screenpatch::core::patches::{commit_refresh, PatchManager, CommitRefreshError, PATCH_BYTE_BUDGET};
use screenpatch::core::refresh::{image_frame, ImageFrame};

fn frame(width: u32, height: u32, fill: u8) -> ImageFrame {
    let len = (width * height * 4) as usize;
    image_frame(width, height, vec![fill; len]).unwrap()
}

fn region(width: u32, height: u32) -> CaptureRect {
    CaptureRect {
        x: 0,
        y: 0,
        width,
        height,
    }
}

#[test]
fn first_refresh_keeps_the_old_current_and_shows_the_new_frame() {
    let mut manager = PatchManager::new();
    let original = frame(2, 2, 1);
    let number = manager.add(region(2, 2), original.clone()).unwrap();
    let fresh = frame(2, 2, 9);

    manager.commit_refresh(number, fresh.clone()).unwrap();

    let stored = manager.get(number).unwrap();
    assert_eq!(stored.frame, fresh);
    assert_eq!(stored.previous.as_ref(), Some(&original));
    assert_eq!(manager.used_bytes(), 32);
}

#[test]
fn second_refresh_drops_the_oldest_frame() {
    let mut manager = PatchManager::new();
    let number = manager.add(region(2, 2), frame(2, 2, 1)).unwrap();
    manager.commit_refresh(number, frame(2, 2, 2)).unwrap();
    let middle = frame(2, 2, 2);
    let newest = frame(2, 2, 3);
    manager.commit_refresh(number, newest.clone()).unwrap();

    let stored = manager.get(number).unwrap();
    assert_eq!(stored.frame, newest);
    assert_eq!(stored.previous.as_ref(), Some(&middle));
    assert_eq!(manager.used_bytes(), 32);
}

#[test]
fn exact_budget_accepts_the_first_refresh_and_one_extra_byte_keeps_the_pair() {
    let current = frame(2, 2, 1);
    let fresh = frame(2, 2, 9);
    let pair = screenpatch::core::patches::FramePair {
        current: current.clone(),
        previous: None,
    };

    let accepted = commit_refresh(&pair, 2, 2, fresh.clone(), PATCH_BYTE_BUDGET - 16).unwrap();
    assert_eq!(accepted.current, fresh);
    assert_eq!(accepted.previous.as_ref(), Some(&current));

    let error = commit_refresh(&pair, 2, 2, fresh, PATCH_BYTE_BUDGET - 15).unwrap_err();
    assert!(matches!(
        error,
        CommitRefreshError::OverBudget {
            frame_bytes: 16,
            used_bytes: used,
        } if used == PATCH_BYTE_BUDGET - 15
    ));
    assert!(pair.previous.is_none());
    assert_eq!(pair.current.pixels[0], 1);
}

#[test]
fn previous_bytes_are_released_before_the_budget_check() {
    let oldest = frame(2, 2, 1);
    let current = frame(2, 2, 2);
    let fresh = frame(2, 2, 3);
    let pair = screenpatch::core::patches::FramePair {
        current: current.clone(),
        previous: Some(oldest.clone()),
    };

    let accepted = commit_refresh(&pair, 2, 2, fresh.clone(), PATCH_BYTE_BUDGET).unwrap();
    assert_eq!(accepted.current, fresh);
    assert_eq!(accepted.previous.as_ref(), Some(&current));

    let error = commit_refresh(&pair, 2, 2, fresh, PATCH_BYTE_BUDGET + 16).unwrap_err();
    assert!(matches!(error, CommitRefreshError::OverBudget { .. }));
    assert_eq!(pair.previous.as_ref(), Some(&oldest));
    assert_eq!(pair.current.pixels[0], 2);
}

#[test]
fn a_frame_of_the_wrong_size_does_not_change_the_pair() {
    let mut manager = PatchManager::new();
    let original = frame(4, 4, 1);
    let number = manager.add(region(4, 4), original.clone()).unwrap();

    let error = manager.commit_refresh(number, frame(4, 3, 9)).unwrap_err();
    assert!(matches!(error, CommitRefreshError::SizeMismatch));
    let stored = manager.get(number).unwrap();
    assert_eq!(stored.frame, original);
    assert!(stored.previous.is_none());
}

#[test]
fn refreshing_one_patch_leaves_the_other_unchanged() {
    let mut manager = PatchManager::new();
    let first = frame(2, 2, 1);
    let second = frame(2, 2, 4);
    let first_number = manager.add(region(2, 2), first).unwrap();
    let second_number = manager.add(region(2, 2), second.clone()).unwrap();
    manager.commit_refresh(second_number, frame(2, 2, 5)).unwrap();

    manager.commit_refresh(first_number, frame(2, 2, 9)).unwrap();

    let other = manager.get(second_number).unwrap();
    assert_eq!(other.frame.pixels[0], 5);
    assert_eq!(other.previous.as_ref().unwrap().pixels[0], second.pixels[0]);
}
