use screenpatch::core::hotkeys::{
    capture_preview, display_chord, format_hotkeys, help_key, help_lines, hotkey_rejection, parse_hotkeys,
    Chord,
    HotkeyAction, HotkeyRejection, Hotkeys, KeyName,
};
use screenpatch::core::language::Language;

const CANONICAL: &str = "\
capture=ctrl+shift+s
inclusive=ctrl+alt+shift+s
refresh=r
frame=1
again=s
close=esc
";

fn bare(key: KeyName) -> Chord {
    Chord {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
        key,
    }
}

#[test]
fn canonical_file_is_the_default_and_round_trips() {
    assert_eq!(parse_hotkeys(CANONICAL), Hotkeys::default());
    assert_eq!(format_hotkeys(&Hotkeys::default()), CANONICAL);
    assert_eq!(display_chord(Hotkeys::default().capture), "Ctrl+Shift+S");
    assert_eq!(display_chord(Hotkeys::default().close), "Esc");
    assert_eq!(capture_preview(None, true, false, true, false), "Ctrl+Shift");
    let shown = Hotkeys::default().capture;
    assert_eq!(capture_preview(Some(shown), true, false, true, false), "Ctrl+Shift+S");
    assert_eq!(capture_preview(Some(shown), false, false, false, false), "Ctrl+Shift+S");
    assert_eq!(capture_preview(Some(shown), false, true, false, false), "Alt");
}

#[test]
fn bad_lines_are_skipped_and_a_duplicate_does_not_replace_another_action() {
    let text = "\
refresh=нет
capture=ctrl+shift+s
inclusive=ctrl+shift+s
refresh=q
";
    let hotkeys = parse_hotkeys(text);
    assert_eq!(hotkeys.refresh, bare(KeyName::Letter('q')));
    assert_eq!(hotkeys.inclusive, Hotkeys::default().inclusive);
    assert_eq!(hotkeys.capture, Hotkeys::default().capture);
}

#[test]
fn rejection_requires_a_global_modifier_and_unique_chords() {
    let current = Hotkeys::default();
    assert_eq!(
        hotkey_rejection(&current, HotkeyAction::Capture, bare(KeyName::Letter('k'))),
        Some(HotkeyRejection::NeedsModifier)
    );
    assert_eq!(
        hotkey_rejection(&current, HotkeyAction::Refresh, bare(KeyName::Letter('r'))),
        None
    );
    assert_eq!(
        hotkey_rejection(&current, HotkeyAction::Refresh, bare(KeyName::Letter('q'))),
        None
    );
    assert_eq!(
        hotkey_rejection(&current, HotkeyAction::Again, current.refresh),
        Some(HotkeyRejection::Duplicate(HotkeyAction::Refresh))
    );
}

#[test]
fn help_keeps_the_fifteen_lines_and_bolds_the_saved_chord() {
    let russian = help_lines(Language::Ru, &Hotkeys::default());
    assert_eq!(
        russian.as_slice(),
        &[
            "Горячие клавиши".to_string(),
            "Ctrl+Shift+S — новый снимок".to_string(),
            "Ctrl+Alt+Shift+S — снимок с окнами программы".to_string(),
            "  ".to_string(),
            "В окне снимка".to_string(),
            "R — обновить".to_string(),
            "1 — размер кадра".to_string(),
            "S — новый снимок".to_string(),
            "Esc — закрыть снимок".to_string(),
            "  ".to_string(),
            "Мышь".to_string(),
            "Правый клик — показать или скрыть кнопки, слайдер и замок".to_string(),
            "Перетаскивание — двигать окно".to_string(),
            "Замок — закрепить положение и размер".to_string(),
            "Слайдер — прозрачность картинки".to_string(),
        ]
    );
    let english = help_lines(Language::En, &Hotkeys::default());
    assert_eq!(english[2], "Ctrl+Alt+Shift+S — capture with the app windows");
    assert_eq!(english[7], "S — new capture");

    let mut changed = Hotkeys::default();
    changed.capture = Chord {
        ctrl: true,
        alt: false,
        shift: true,
        win: false,
        key: KeyName::Letter('k'),
    };
    let line = &help_lines(Language::Ru, &changed)[1];
    assert!(line.starts_with("Ctrl+Shift+K — "));
    assert_eq!(help_key(line), Some("Ctrl+Shift+K"));
    assert_eq!(help_key("Перетаскивание — двигать окно"), None);
    assert_eq!(help_key("Drag — move the window"), None);
}
