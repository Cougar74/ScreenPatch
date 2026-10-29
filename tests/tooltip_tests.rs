use std::time::{Duration, Instant};

use screenpatch::core::commands::PanelAction;
use screenpatch::core::language::{text, Language};
use screenpatch::core::tooltip::{
    action_tip, button_tip, lock_tip, note_hover, tooltip_client_size, tooltip_due, tooltip_origin,
};

#[test]
fn button_tips_use_the_existing_names() {
    let cases = [
        (PanelAction::Refresh, "Обновить", "Refresh"),
        (PanelAction::BeforeAfter, "До/после", "Before/after"),
        (PanelAction::Copy, "Копировать", "Copy"),
        (PanelAction::Save, "Сохранить", "Save"),
        (PanelAction::Close, "Закрыть", "Close"),
    ];
    for (action, russian, english) in cases {
        let phrase = button_tip(action).unwrap();
        assert_eq!(text(Language::Ru, phrase), russian);
        assert_eq!(text(Language::En, phrase), english);
    }
    assert_eq!(button_tip(PanelAction::Drag), None);
    assert_eq!(button_tip(PanelAction::Resize), None);
}

#[test]
fn tooltip_waits_200_ms_and_keeps_the_same_button() {
    let started = Instant::now();
    let hover = note_hover(None, 7u64, PanelAction::Refresh, started);
    assert!(!tooltip_due(&hover, started + Duration::from_millis(199)));
    assert!(tooltip_due(&hover, started + Duration::from_millis(200)));

    let mut shown = hover;
    shown.shown = true;
    let later = started + Duration::from_millis(50);
    let same = note_hover(Some(shown), 7u64, PanelAction::Refresh, later);
    assert_eq!(same.started, started);
    assert!(same.shown);

    let other = note_hover(Some(shown), 7u64, PanelAction::Copy, later);
    assert_eq!(other.started, later);
    assert!(!other.shown);
    let other_window = note_hover(Some(shown), 8u64, PanelAction::Refresh, later);
    assert_eq!(other_window.started, later);
    assert!(!other_window.shown);
}

#[test]
fn tooltip_origin_follows_the_cursor_and_stays_in_the_work_area() {
    let work = Some((0, 0, 1920, 1080));
    assert_eq!(tooltip_origin(100, 100, 80, 24, work), (116, 120));
    assert_eq!(tooltip_origin(1900, 100, 80, 24, work), (1804, 120));
    assert_eq!(tooltip_origin(100, 1060, 80, 24, work), (116, 1028));
    assert_eq!(tooltip_origin(1900, 1060, 80, 24, work), (1804, 1028));
    assert_eq!(tooltip_origin(20, 20, 30, 24, Some((0, 0, 40, 40))), (10, 16));
    assert_eq!(tooltip_origin(100, 100, 80, 24, None), (116, 120));
}

#[test]
fn tooltip_client_size_adds_padding_on_both_sides() {
    assert_eq!(tooltip_client_size(40, 16, 8, 4), (56, 24));
}

#[test]
fn lock_tip_follows_the_flag_and_is_not_a_short_button_name() {
    assert_eq!(button_tip(PanelAction::Lock), None);
    assert_eq!(text(Language::Ru, lock_tip(false)), "Закрепить");
    assert_eq!(text(Language::En, lock_tip(false)), "Pin");
    assert_eq!(text(Language::Ru, lock_tip(true)), "Открепить");
    assert_eq!(text(Language::En, lock_tip(true)), "Unpin");
    assert_eq!(action_tip(PanelAction::Lock, false), Some(lock_tip(false)));
    assert_eq!(action_tip(PanelAction::Lock, true), Some(lock_tip(true)));
    assert_eq!(action_tip(PanelAction::Refresh, true), button_tip(PanelAction::Refresh));
    assert_eq!(action_tip(PanelAction::Drag, false), None);
}
