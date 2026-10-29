use screenpatch::core::language::{
    help_key, help_lines, language_choice, parse_saved_language, text, Language, BUTTON_ICON_FAILED,
    LANGUAGE_NAME_EN, LANGUAGE_NAME_RU, MONITOR_MISSING, TRAY_HELP, TRAY_LANGUAGE, TRAY_NEW,
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
fn tray_language_and_help_use_the_approved_labels() {
    assert_eq!(text(Language::Ru, TRAY_LANGUAGE), "Язык");
    assert_eq!(text(Language::En, TRAY_LANGUAGE), "Language");
    assert_eq!(text(Language::Ru, TRAY_HELP), "Справка");
    assert_eq!(text(Language::En, TRAY_HELP), "Help");
    assert_eq!(LANGUAGE_NAME_RU, "Русский");
    assert_eq!(LANGUAGE_NAME_EN, "English");
}

#[test]
fn help_lines_list_the_approved_hotkeys() {
    assert_eq!(
        help_lines(Language::Ru),
        &[
            "Горячие клавиши",
            "Ctrl+Shift+S — новый снимок",
            "Ctrl+Alt+Shift+S — снимок с окнами программы",
            "  ",
            "В окне снимка",
            "R — обновить",
            "1 — размер кадра",
            "S — новый снимок",
            "Esc — закрыть снимок",
            "  ",
            "Мышь",
            "Правый клик — показать или скрыть кнопки",
            "Перетаскивание — двигать окно",
        ]
    );
    assert_eq!(
        help_lines(Language::En),
        &[
            "Hotkeys",
            "Ctrl+Shift+S — new capture",
            "Ctrl+Alt+Shift+S — capture with the app windows",
            "  ",
            "In a capture window",
            "R — refresh",
            "1 — frame size",
            "S — new capture",
            "Esc — close the capture",
            "  ",
            "Mouse",
            "Right-click — show or hide the buttons",
            "Drag — move the window",
        ]
    );
}

#[test]
fn help_key_is_only_the_shortcut_before_the_dash() {
    assert_eq!(
        help_key("Ctrl+Shift+S — новый снимок"),
        Some("Ctrl+Shift+S")
    );
    assert_eq!(
        help_key("Ctrl+Alt+Shift+S — снимок с окнами программы"),
        Some("Ctrl+Alt+Shift+S")
    );
    assert_eq!(help_key("R — обновить"), Some("R"));
    assert_eq!(help_key("1 — frame size"), Some("1"));
    assert_eq!(help_key("S — new capture"), Some("S"));
    assert_eq!(help_key("Esc — закрыть снимок"), Some("Esc"));
    assert_eq!(help_key("Мышь"), None);
    assert_eq!(
        help_key("Правый клик — показать или скрыть кнопки"),
        None
    );
    assert_eq!(help_key("Drag — move the window"), None);
}

#[test]
fn inclusive_capture_phrases_match_the_spec() {
    use screenpatch::core::language::{
        HOTKEY_INCLUSIVE_BUSY, INCLUDE_CAPTURE_FAILED, INCLUDE_TOOLTIP_FAILED,
        RESTORE_CAPTURE_FAILED, RESTORE_TOOLTIP_FAILED,
    };
    assert_eq!(
        text(Language::Ru, HOTKEY_INCLUSIVE_BUSY),
        "сочетание Ctrl+Alt+Shift+S занято: {err}"
    );
    assert_eq!(
        text(Language::En, HOTKEY_INCLUSIVE_BUSY),
        "Ctrl+Alt+Shift+S is already taken: {err}"
    );
    assert_eq!(
        text(Language::Ru, INCLUDE_CAPTURE_FAILED),
        "не удалось включить снимок {number} в кадр"
    );
    assert_eq!(
        text(Language::En, INCLUDE_CAPTURE_FAILED),
        "could not include capture {number} in the shot"
    );
    assert_eq!(
        text(Language::Ru, INCLUDE_TOOLTIP_FAILED),
        "не удалось включить подсказку в кадр"
    );
    assert_eq!(
        text(Language::En, INCLUDE_TOOLTIP_FAILED),
        "could not include the tooltip in the shot"
    );
    assert_eq!(
        text(Language::Ru, RESTORE_CAPTURE_FAILED),
        "не удалось снова исключить снимок {number}"
    );
    assert_eq!(
        text(Language::En, RESTORE_CAPTURE_FAILED),
        "could not exclude capture {number} again"
    );
    assert_eq!(
        text(Language::Ru, RESTORE_TOOLTIP_FAILED),
        "не удалось снова исключить подсказку"
    );
    assert_eq!(
        text(Language::En, RESTORE_TOOLTIP_FAILED),
        "could not exclude the tooltip again"
    );
}

#[test]
fn language_choice_ignores_the_current_language() {
    assert_eq!(language_choice(Language::Ru, Language::Ru), None);
    assert_eq!(language_choice(Language::En, Language::En), None);
    assert_eq!(
        language_choice(Language::Ru, Language::En),
        Some(Language::En)
    );
    assert_eq!(
        language_choice(Language::En, Language::Ru),
        Some(Language::Ru)
    );
}

#[test]
fn russian_monitor_missing_keeps_the_checklist_words() {
    assert!(text(Language::Ru, MONITOR_MISSING).contains("монитор больше не подключён"));
    let english = text(Language::En, MONITOR_MISSING);
    assert!(english.contains("no longer connected"));
    assert!(english.chars().all(|ch| !('\u{0400}'..='\u{04FF}').contains(&ch)));
}

#[test]
fn button_icon_failure_names_the_read_error() {
    assert_eq!(
        text(Language::Ru, BUTTON_ICON_FAILED),
        "не удалось прочитать иконку кнопки: {err}"
    );
    assert_eq!(
        text(Language::En, BUTTON_ICON_FAILED),
        "could not read the button icon: {err}"
    );
}
