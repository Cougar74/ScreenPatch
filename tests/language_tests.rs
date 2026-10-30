use screenpatch::core::hotkeys::{help_key, help_lines, Hotkeys};
use screenpatch::core::language::{
    language_choice, parse_saved_language, render, text, Language, BUTTON_ICON_FAILED, HOTKEY_AGAIN,
    HOTKEY_CHANGE, HOTKEY_CLOSE, HOTKEY_DUPLICATE, HOTKEY_FRAME, HOTKEY_INCLUSIVE, HOTKEY_INCOMPLETE,
    HOTKEY_NEEDS_MODIFIER, HOTKEY_READ_FAILED, HOTKEY_REGISTER_FAILED, HOTKEY_WRITE_FAILED,
    LANGUAGE_NAME_EN, LANGUAGE_NAME_RU, MONITOR_MISSING, TRAY_HELP, TRAY_HOTKEYS, TRAY_LANGUAGE,
    TRAY_NEW,
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

fn help_lines_match(actual: [String; 15], expected: &[&str]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual, expected);
    }
}

#[test]
fn help_lines_list_the_approved_hotkeys() {
    help_lines_match(
        help_lines(Language::Ru, &Hotkeys::default()),
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
            "Правый клик — показать или скрыть кнопки, слайдер и замок",
            "Перетаскивание — двигать окно",
            "Замок — закрепить положение и размер",
            "Слайдер — прозрачность картинки",
        ]
    );
    help_lines_match(
        help_lines(Language::En, &Hotkeys::default()),
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
            "Right-click — show or hide the buttons, the slider, and the lock",
            "Drag — move the window",
            "Lock — pin position and size",
            "Slider — picture opacity",
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
        help_key("Правый клик — показать или скрыть кнопки, слайдер и замок"),
        None
    );
    assert_eq!(help_key("Замок — закрепить положение и размер"), None);
    assert_eq!(help_key("Lock — pin position and size"), None);
    assert_eq!(help_key("Drag — move the window"), None);
}

#[test]
fn show_failed_phrase_matches_the_spec() {
    use screenpatch::core::language::SHOW_FAILED;
    assert_eq!(text(Language::Ru, SHOW_FAILED), "не удалось показать снимок {number}");
    assert_eq!(text(Language::En, SHOW_FAILED), "could not show capture {number}");
    assert_eq!(help_key("Слайдер — прозрачность картинки"), None);
    assert_eq!(help_key("Slider — picture opacity"), None);
    assert_eq!(
        help_key("Правый клик — показать или скрыть кнопки, слайдер и замок"),
        None
    );
    assert_eq!(help_key("Lock — pin position and size"), None);
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

#[test]
fn hotkey_phrases_use_the_approved_words() {
    assert_eq!(text(Language::Ru, TRAY_HOTKEYS), "Сочетания");
    assert_eq!(text(Language::En, TRAY_HOTKEYS), "Shortcuts");
    assert_eq!(text(Language::Ru, HOTKEY_CHANGE), "Изменить");
    assert_eq!(text(Language::En, HOTKEY_CHANGE), "Change");
    assert_eq!(text(Language::Ru, HOTKEY_INCOMPLETE), "Сочетание не закончено.");
    assert_eq!(text(Language::En, HOTKEY_INCOMPLETE), "The shortcut is incomplete.");
    assert_eq!(
        text(Language::Ru, HOTKEY_NEEDS_MODIFIER),
        "Глобальному сочетанию нужен Ctrl, Alt, Shift или Win."
    );
    assert_eq!(
        text(Language::En, HOTKEY_NEEDS_MODIFIER),
        "A global shortcut needs Ctrl, Alt, Shift, or Win."
    );
    assert_eq!(
        render(Language::Ru, HOTKEY_DUPLICATE, &[("action", "Обновить")]),
        "Это сочетание уже занято: Обновить."
    );
    assert_eq!(
        render(Language::En, HOTKEY_DUPLICATE, &[("action", "Refresh")]),
        "This shortcut is already used by Refresh."
    );
    assert_eq!(text(Language::Ru, HOTKEY_REGISTER_FAILED), "Система не отдала это сочетание.");
    assert_eq!(
        text(Language::En, HOTKEY_REGISTER_FAILED),
        "The system did not accept this shortcut."
    );
    assert_eq!(
        render(Language::Ru, HOTKEY_READ_FAILED, &[("err", "denied")]),
        "не удалось прочитать сочетания: denied"
    );
    assert_eq!(
        render(Language::En, HOTKEY_WRITE_FAILED, &[("err", "full")]),
        "Could not save the shortcuts: full."
    );
    assert_eq!(text(Language::Ru, HOTKEY_INCLUSIVE), "Снимок с окнами программы");
    assert_eq!(text(Language::En, HOTKEY_INCLUSIVE), "Capture with the app windows");
    assert_eq!(text(Language::Ru, HOTKEY_FRAME), "Размер кадра");
    assert_eq!(text(Language::En, HOTKEY_FRAME), "Frame size");
    assert_eq!(text(Language::Ru, HOTKEY_AGAIN), "Новый снимок из окна");
    assert_eq!(text(Language::En, HOTKEY_AGAIN), "New capture from the window");
    assert_eq!(text(Language::Ru, HOTKEY_CLOSE), "Закрыть снимок");
    assert_eq!(text(Language::En, HOTKEY_CLOSE), "Close the capture");
}
