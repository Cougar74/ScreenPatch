use crate::core::language::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    Capture,
    Inclusive,
    Refresh,
    Frame,
    Again,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyName {
    Letter(char),
    Digit(u8),
    Function(u8),
    Space,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Esc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub key: KeyName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkeys {
    pub capture: Chord,
    pub inclusive: Chord,
    pub refresh: Chord,
    pub frame: Chord,
    pub again: Chord,
    pub close: Chord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyRejection {
    NeedsModifier,
    Duplicate(HotkeyAction),
}

fn bare(key: KeyName) -> Chord {
    Chord {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
        key,
    }
}

fn chord(ctrl: bool, alt: bool, shift: bool, win: bool, key: KeyName) -> Chord {
    Chord {
        ctrl,
        alt,
        shift,
        win,
        key,
    }
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            capture: chord(true, false, true, false, KeyName::Letter('s')),
            inclusive: chord(true, true, true, false, KeyName::Letter('s')),
            refresh: bare(KeyName::Letter('r')),
            frame: bare(KeyName::Digit(1)),
            again: bare(KeyName::Letter('s')),
            close: bare(KeyName::Esc),
        }
    }
}

impl Hotkeys {
    pub fn get(&self, action: HotkeyAction) -> Chord {
        match action {
            HotkeyAction::Capture => self.capture,
            HotkeyAction::Inclusive => self.inclusive,
            HotkeyAction::Refresh => self.refresh,
            HotkeyAction::Frame => self.frame,
            HotkeyAction::Again => self.again,
            HotkeyAction::Close => self.close,
        }
    }

    pub fn assign(&mut self, action: HotkeyAction, chord: Chord) {
        self.set(action, chord);
    }

    fn set(&mut self, action: HotkeyAction, chord: Chord) {
        match action {
            HotkeyAction::Capture => self.capture = chord,
            HotkeyAction::Inclusive => self.inclusive = chord,
            HotkeyAction::Refresh => self.refresh = chord,
            HotkeyAction::Frame => self.frame = chord,
            HotkeyAction::Again => self.again = chord,
            HotkeyAction::Close => self.close = chord,
        }
    }
}

const ACTIONS: [(&str, HotkeyAction); 6] = [
    ("capture", HotkeyAction::Capture),
    ("inclusive", HotkeyAction::Inclusive),
    ("refresh", HotkeyAction::Refresh),
    ("frame", HotkeyAction::Frame),
    ("again", HotkeyAction::Again),
    ("close", HotkeyAction::Close),
];

pub fn format_hotkeys(hotkeys: &Hotkeys) -> String {
    let mut text = String::new();
    for (name, action) in ACTIONS {
        text.push_str(name);
        text.push('=');
        text.push_str(&file_chord(hotkeys.get(action)));
        text.push('\n');
    }
    text
}

pub fn parse_hotkeys(text: &str) -> Hotkeys {
    let mut hotkeys = Hotkeys::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, chord)) = line.split_once('=') else {
            continue;
        };
        let Some(action) = ACTIONS.iter().find(|(label, _)| *label == name).map(|(_, action)| *action) else {
            continue;
        };
        let Some(chord) = parse_chord(chord) else {
            continue;
        };
        if hotkey_rejection(&hotkeys, action, chord).is_some() {
            continue;
        }
        hotkeys.set(action, chord);
    }
    hotkeys
}

pub fn display_modifiers(ctrl: bool, alt: bool, shift: bool, win: bool) -> String {
    let mut parts = Vec::new();
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    if win {
        parts.push("Win");
    }
    parts.join("+")
}

pub fn display_chord(chord: Chord) -> String {
    let modifiers = display_modifiers(chord.ctrl, chord.alt, chord.shift, chord.win);
    let key = display_key(chord.key);
    if modifiers.is_empty() {
        key.to_string()
    } else {
        format!("{modifiers}+{key}")
    }
}

pub fn capture_preview(shown: Option<Chord>, ctrl: bool, alt: bool, shift: bool, win: bool) -> String {
    let live = display_modifiers(ctrl, alt, shift, win);
    let held = ctrl || alt || shift || win;
    if let Some(chord) = shown {
        let same = chord.ctrl == ctrl && chord.alt == alt && chord.shift == shift && chord.win == win;
        if held && !same {
            return live;
        }
        return display_chord(chord);
    }
    live
}

pub fn hotkey_rejection(
    current: &Hotkeys,
    action: HotkeyAction,
    chord: Chord,
) -> Option<HotkeyRejection> {
    let global = matches!(action, HotkeyAction::Capture | HotkeyAction::Inclusive);
    if global && !chord.ctrl && !chord.alt && !chord.shift && !chord.win {
        return Some(HotkeyRejection::NeedsModifier);
    }
    for (name, other) in ACTIONS {
        let _ = name;
        if other == action {
            continue;
        }
        if current.get(other) == chord {
            return Some(HotkeyRejection::Duplicate(other));
        }
    }
    None
}

pub fn help_lines(language: Language, hotkeys: &Hotkeys) -> [String; 15] {
    let shortcut = |chord: Chord, ru: &str, en: &str| {
        let label = match language {
            Language::Ru => ru,
            Language::En => en,
        };
        format!("{} — {label}", display_chord(chord))
    };
    let plain = |ru: &str, en: &str| {
        match language {
            Language::Ru => ru,
            Language::En => en,
        }
        .to_string()
    };
    [
        plain("Горячие клавиши", "Hotkeys"),
        shortcut(hotkeys.capture, "новый снимок", "new capture"),
        shortcut(hotkeys.inclusive, "снимок с окнами программы", "capture with the app windows"),
        plain("  ", "  "),
        plain("В окне снимка", "In a capture window"),
        shortcut(hotkeys.refresh, "обновить", "refresh"),
        shortcut(hotkeys.frame, "размер кадра", "frame size"),
        shortcut(hotkeys.again, "новый снимок", "new capture"),
        shortcut(hotkeys.close, "закрыть снимок", "close the capture"),
        plain("  ", "  "),
        plain("Мышь", "Mouse"),
        plain(
            "Правый клик — показать или скрыть кнопки, слайдер и замок",
            "Right-click — show or hide the buttons, the slider, and the lock",
        ),
        plain("Перетаскивание — двигать окно", "Drag — move the window"),
        plain("Замок — закрепить положение и размер", "Lock — pin position and size"),
        plain("Слайдер — прозрачность картинки", "Slider — picture opacity"),
    ]
}

pub fn help_key(line: &str) -> Option<&str> {
    let (left, _) = line.split_once(" — ")?;
    if is_display_chord(left) {
        Some(left)
    } else {
        None
    }
}

fn file_chord(chord: Chord) -> String {
    let mut parts = Vec::new();
    if chord.ctrl {
        parts.push("ctrl".to_string());
    }
    if chord.alt {
        parts.push("alt".to_string());
    }
    if chord.shift {
        parts.push("shift".to_string());
    }
    if chord.win {
        parts.push("win".to_string());
    }
    parts.push(file_key(chord.key));
    parts.join("+")
}

fn file_key(key: KeyName) -> String {
    match key {
        KeyName::Letter(letter) => letter.to_ascii_lowercase().to_string(),
        KeyName::Digit(digit) => digit.to_string(),
        KeyName::Function(number) => format!("f{number}"),
        KeyName::Space => "space".to_string(),
        KeyName::Tab => "tab".to_string(),
        KeyName::Up => "up".to_string(),
        KeyName::Down => "down".to_string(),
        KeyName::Left => "left".to_string(),
        KeyName::Right => "right".to_string(),
        KeyName::Esc => "esc".to_string(),
    }
}

fn display_key(key: KeyName) -> &'static str {
    match key {
        KeyName::Letter(letter) => match letter.to_ascii_uppercase() {
            'A' => "A",
            'B' => "B",
            'C' => "C",
            'D' => "D",
            'E' => "E",
            'F' => "F",
            'G' => "G",
            'H' => "H",
            'I' => "I",
            'J' => "J",
            'K' => "K",
            'L' => "L",
            'M' => "M",
            'N' => "N",
            'O' => "O",
            'P' => "P",
            'Q' => "Q",
            'R' => "R",
            'S' => "S",
            'T' => "T",
            'U' => "U",
            'V' => "V",
            'W' => "W",
            'X' => "X",
            'Y' => "Y",
            'Z' => "Z",
            _ => "?",
        },
        KeyName::Digit(0) => "0",
        KeyName::Digit(1) => "1",
        KeyName::Digit(2) => "2",
        KeyName::Digit(3) => "3",
        KeyName::Digit(4) => "4",
        KeyName::Digit(5) => "5",
        KeyName::Digit(6) => "6",
        KeyName::Digit(7) => "7",
        KeyName::Digit(8) => "8",
        KeyName::Digit(9) => "9",
        KeyName::Digit(_) => "?",
        KeyName::Function(1) => "F1",
        KeyName::Function(2) => "F2",
        KeyName::Function(3) => "F3",
        KeyName::Function(4) => "F4",
        KeyName::Function(5) => "F5",
        KeyName::Function(6) => "F6",
        KeyName::Function(7) => "F7",
        KeyName::Function(8) => "F8",
        KeyName::Function(9) => "F9",
        KeyName::Function(10) => "F10",
        KeyName::Function(11) => "F11",
        KeyName::Function(12) => "F12",
        KeyName::Function(_) => "?",
        KeyName::Space => "Space",
        KeyName::Tab => "Tab",
        KeyName::Up => "Up",
        KeyName::Down => "Down",
        KeyName::Left => "Left",
        KeyName::Right => "Right",
        KeyName::Esc => "Esc",
    }
}

fn parse_chord(text: &str) -> Option<Chord> {
    let mut parts = text.split('+').filter(|part| !part.is_empty());
    let mut words = Vec::new();
    for part in parts.by_ref() {
        words.push(part);
    }
    let key_word = words.pop()?;
    let key = parse_key(key_word)?;
    let mut chord = bare(key);
    for word in words {
        match word {
            "ctrl" => chord.ctrl = true,
            "alt" => chord.alt = true,
            "shift" => chord.shift = true,
            "win" => chord.win = true,
            _ => return None,
        }
    }
    Some(chord)
}

fn parse_key(word: &str) -> Option<KeyName> {
    if word.len() == 1 {
        let ch = word.chars().next()?;
        if ch.is_ascii_lowercase() {
            return Some(KeyName::Letter(ch));
        }
        if ch.is_ascii_digit() {
            return Some(KeyName::Digit(ch.to_digit(10)? as u8));
        }
    }
    if let Some(number) = word.strip_prefix('f') {
        if let Ok(number) = number.parse::<u8>() {
            if (1..=12).contains(&number) {
                return Some(KeyName::Function(number));
            }
        }
    }
    Some(match word {
        "space" => KeyName::Space,
        "tab" => KeyName::Tab,
        "up" => KeyName::Up,
        "down" => KeyName::Down,
        "left" => KeyName::Left,
        "right" => KeyName::Right,
        "esc" => KeyName::Esc,
        _ => return None,
    })
}

fn is_display_chord(text: &str) -> bool {
    let mut parts: Vec<&str> = text.split('+').collect();
    let Some(key) = parts.pop() else {
        return false;
    };
    if display_key_token(key).is_none() {
        return false;
    }
    const ORDER: [&str; 4] = ["Ctrl", "Alt", "Shift", "Win"];
    let mut index = 0;
    for part in parts {
        while index < ORDER.len() && ORDER[index] != part {
            index += 1;
        }
        if index == ORDER.len() {
            return false;
        }
        index += 1;
    }
    true
}

fn display_key_token(token: &str) -> Option<KeyName> {
    if token.len() == 1 {
        let ch = token.chars().next()?;
        if ch.is_ascii_uppercase() {
            return Some(KeyName::Letter(ch.to_ascii_lowercase()));
        }
        if ch.is_ascii_digit() {
            return Some(KeyName::Digit(ch.to_digit(10)? as u8));
        }
    }
    if let Some(number) = token.strip_prefix('F') {
        if let Ok(number) = number.parse::<u8>() {
            if (1..=12).contains(&number) {
                return Some(KeyName::Function(number));
            }
        }
    }
    Some(match token {
        "Space" => KeyName::Space,
        "Tab" => KeyName::Tab,
        "Up" => KeyName::Up,
        "Down" => KeyName::Down,
        "Left" => KeyName::Left,
        "Right" => KeyName::Right,
        "Esc" => KeyName::Esc,
        _ => return None,
    })
}
