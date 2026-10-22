use screenpatch::core::language::{
    parse_saved_language, text, Language, MONITOR_MISSING, TRAY_NEW, TRAY_OTHER_LANGUAGE,
};

#[test]
fn saved_language_accepts_ru_and_en_with_surrounding_space() {
    assert_eq!(parse_saved_language("ru"), Some(Language::Ru));
    assert_eq!(parse_saved_language("  ru\n"), Some(Language::Ru));
    assert_eq!(parse_saved_language("en"), Some(Language::En));
}

#[test]
fn saved_language_rejects_empty_and_unknown_words() {
    assert_eq!(parse_saved_language(""), None);
    assert_eq!(parse_saved_language("de"), None);
    assert_eq!(parse_saved_language("russian"), None);
}

#[test]
fn tray_new_capture_uses_the_approved_labels() {
    assert_eq!(text(Language::Ru, TRAY_NEW), "Новый снимок");
    assert_eq!(text(Language::En, TRAY_NEW), "New capture");
}

#[test]
fn other_language_item_names_the_language_being_switched_to() {
    assert_eq!(text(Language::Ru, TRAY_OTHER_LANGUAGE), "Переключить на English");
    assert_eq!(text(Language::En, TRAY_OTHER_LANGUAGE), "Change to Русский");
}

#[test]
fn russian_monitor_missing_keeps_the_checklist_words() {
    assert!(text(Language::Ru, MONITOR_MISSING).contains("монитор больше не подключён"));
    let english = text(Language::En, MONITOR_MISSING);
    assert!(english.contains("no longer connected"));
    assert!(english.chars().all(|ch| !('\u{0400}'..='\u{04FF}').contains(&ch)));
}
