#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Ru,
    En,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phrase {
    ru: &'static str,
    en: &'static str,
}

pub fn text(language: Language, phrase: Phrase) -> &'static str {
    match language {
        Language::Ru => phrase.ru,
        Language::En => phrase.en,
    }
}

pub fn render(language: Language, phrase: Phrase, pairs: &[(&str, &str)]) -> String {
    let mut rendered = text(language, phrase).to_string();
    for (key, value) in pairs {
        rendered = rendered.replace(&format!("{{{key}}}"), value);
    }
    rendered
}

pub fn parse_saved_language(text: &str) -> Option<Language> {
    match text.trim() {
        "ru" => Some(Language::Ru),
        "en" => Some(Language::En),
        _ => None,
    }
}

pub const TRAY_NEW: Phrase = Phrase { ru: "Новый снимок", en: "New capture" };
pub const TRAY_SHOW: Phrase = Phrase { ru: "Показать все", en: "Show all" };
pub const TRAY_HIDE: Phrase = Phrase { ru: "Скрыть все", en: "Hide all" };
pub const TRAY_CLOSE_ALL: Phrase = Phrase { ru: "Закрыть все снимки", en: "Close all captures" };
pub const TRAY_OTHER_LANGUAGE: Phrase = Phrase {
    ru: "Переключить на English",
    en: "Change to Русский",
};
pub const TRAY_EXIT: Phrase = Phrase { ru: "Выход", en: "Exit" };
pub const STRIP_REFRESH: Phrase = Phrase { ru: "Обновить", en: "Refresh" };
pub const STRIP_BEFORE_AFTER: Phrase = Phrase { ru: "До/после", en: "Before/after" };
pub const STRIP_COPY: Phrase = Phrase { ru: "Копировать", en: "Copy" };
pub const STRIP_SAVE: Phrase = Phrase { ru: "Сохранить", en: "Save" };
pub const STRIP_CLOSE: Phrase = Phrase { ru: "Закрыть", en: "Close" };
pub const CLOSE_ALL_QUESTION: Phrase = Phrase {
    ru: "Закрыть все снимки и освободить их кадры?",
    en: "Close all captures and release their frames?",
};
pub const ALREADY_RUNNING: Phrase = Phrase { ru: "ScreenPatch уже запущен", en: "ScreenPatch is already running" };
pub const REFRESH_IN_PROGRESS: Phrase = Phrase { ru: "обновление уже выполняется", en: "a refresh is already running" };
pub const WINDOW_LIMIT: Phrase = Phrase {
    ru: "лимит окон достигнут: уже открыто 10 снимков",
    en: "the window limit has been reached: 10 captures are already open",
};
pub const MONITOR_MISSING: Phrase = Phrase {
    ru: "монитор больше не подключён",
    en: "the monitor is no longer connected",
};
pub const GEOMETRY_CHANGED: Phrase = Phrase {
    ru: "геометрия монитора изменилась",
    en: "the monitor geometry changed",
};
pub const BAD_WINDOW_SCALE: Phrase = Phrase {
    ru: "масштаб окна некорректен, размер снимка {number} не изменён",
    en: "the window scale is invalid, capture {number} was not resized",
};
pub const POSITION_UNREAD: Phrase = Phrase {
    ru: "не удалось прочитать положение снимка {number}, окно оставлено на месте",
    en: "could not read the position of capture {number}, the window was left in place",
};
pub const WORK_AREA_UNREAD: Phrase = Phrase {
    ru: "не удалось прочитать рабочую область снимка {number}, окно оставлено на месте",
    en: "could not read the work area of capture {number}, the window was left in place",
};
pub const WORK_AREA_UNREAD_ERR: Phrase = Phrase {
    ru: "не удалось прочитать рабочую область снимка {number}: {err}, окно оставлено на месте",
    en: "could not read the work area of capture {number}: {err}, the window was left in place",
};
pub const CROP_FAILED: Phrase = Phrase {
    ru: "не удалось вырезать область, монитор id={id}, прямоугольник=({x},{y},{w}x{h})",
    en: "could not crop the region, monitor id={id}, rectangle=({x},{y},{w}x{h})",
};
pub const PATCH_WINDOW_FAILED: Phrase = Phrase {
    ru: "не удалось создать окно снимка: {err}, монитор id={id}, прямоугольник=({x},{y},{w}x{h})",
    en: "could not create the capture window: {err}, monitor id={id}, rectangle=({x},{y},{w}x{h})",
};
pub const REFRESH_BUDGET: Phrase = Phrase {
    ru: "не удалось обновить снимок {number}: кадр {w}x{h} занимает {bytes} байт, уже занято {used} байт, монитор id={id}",
    en: "could not refresh capture {number}: frame {w}x{h} uses {bytes} bytes, {used} bytes are already used, monitor id={id}",
};
pub const REFRESH_SIZE: Phrase = Phrase {
    ru: "не удалось обновить снимок {number}: размер кадра {w}x{h} не совпал с прямоугольником {rw}x{rh}, монитор id={id}",
    en: "could not refresh capture {number}: frame {w}x{h} does not match rectangle {rw}x{rh}, monitor id={id}",
};
pub const COPY_FAILED: Phrase = Phrase {
    ru: "не удалось скопировать снимок {number}: {err}",
    en: "could not copy capture {number}: {err}",
};
pub const SAVE_FAILED: Phrase = Phrase {
    ru: "не удалось сохранить снимок {number}: {err}",
    en: "could not save capture {number}: {err}",
};
pub const WINDOW_ICON_FAILED: Phrase = Phrase {
    ru: "не удалось поставить значок окна: {err}",
    en: "could not set the window icon: {err}",
};
pub const LAYERS_FAILED: Phrase = Phrase {
    ru: "не удалось открыть слои выделения: {err}",
    en: "could not open the selection layers: {err}",
};
pub const SELECTION_FAILED: Phrase = Phrase {
    ru: "не удалось начать новое выделение: {err}",
    en: "could not start a new selection: {err}",
};
pub const HOTKEY_BUSY: Phrase = Phrase {
    ru: "сочетание Ctrl+Shift+S занято, новый снимок доступен из значка: {err}",
    en: "Ctrl+Shift+S is already taken, a new capture is available from the tray icon: {err}",
};
pub const REFRESH_CANCELLED: Phrase = Phrase {
    ru: "обновление отменено: {reason}, монитор id={id}, прямоугольник=({x},{y},{w}x{h})",
    en: "refresh cancelled: {reason}, monitor id={id}, rectangle=({x},{y},{w}x{h})",
};
pub const MONITOR_LOG: Phrase = Phrase {
    ru: "монитор id={id} начало=({x},{y}) размер={w}x{h} масштаб={scale} прямоугольник=({rx},{ry},{rw}x{rh}) исключение={excluded}",
    en: "monitor id={id} origin=({x},{y}) size={w}x{h} scale={scale} rectangle=({rx},{ry},{rw}x{rh}) excluded={excluded}",
};
pub const EXCLUDED_HIDDEN: Phrase = Phrase { ru: "скрытие", en: "hidden" };
pub const EXCLUDED_AFFINITY: Phrase = Phrase { ru: "display-affinity", en: "display-affinity" };
pub const SINGLE_INSTANCE_FAILED: Phrase = Phrase {
    ru: "не удалось проверить единственный экземпляр: {err}",
    en: "could not check the single instance: {err}",
};
pub const LANGUAGE_READ_FAILED: Phrase = Phrase {
    ru: "не удалось прочитать сохранённый язык: {err}",
    en: "could not read the saved language: {err}",
};
pub const LANGUAGE_SAVE_FAILED: Phrase = Phrase {
    ru: "не удалось запомнить язык: {err}",
    en: "could not save the language choice: {err}",
};
pub const NO_MONITORS: Phrase = Phrase { ru: "мониторы не найдены", en: "no monitors were found" };
pub const MONITORS_NOT_FOUND: Phrase = Phrase {
    ru: "мониторы не найдены: {err}",
    en: "no monitors were found: {err}",
};
pub const BAD_MONITOR_SCALE: Phrase = Phrase {
    ru: "масштаб монитора {id} некорректен",
    en: "the scale of monitor {id} is invalid",
};
pub const BAD_MONITOR_SIZE: Phrase = Phrase {
    ru: "размер монитора {id} некорректен",
    en: "the size of monitor {id} is invalid",
};
pub const CAPTURE_MONITOR_FAILED: Phrase = Phrase {
    ru: "не удалось снять монитор {id}: {err}",
    en: "could not capture monitor {id}: {err}",
};
pub const LAYERS_ALREADY: Phrase = Phrase { ru: "слои уже созданы", en: "the layers have already been created" };
pub const OVERLAY_CREATE_FAILED: Phrase = Phrase {
    ru: "не удалось создать слой монитора {id}: {err}",
    en: "could not create the layer for monitor {id}: {err}",
};
pub const OVERLAY_BUFFER_FAILED: Phrase = Phrase {
    ru: "не удалось создать буфер слоя {id}: {err}",
    en: "could not create the buffer for layer {id}: {err}",
};
pub const OVERLAY_SURFACE_FAILED: Phrase = Phrase {
    ru: "не удалось создать поверхность слоя {id}: {err}",
    en: "could not create the surface for layer {id}: {err}",
};
pub const WINDOW_POS_FAILED: Phrase = Phrase {
    ru: "не удалось перевести положение окна монитора {id}",
    en: "could not convert the window position for monitor {id}",
};
pub const WINDOW_CREATE_FAILED: Phrase = Phrase {
    ru: "не удалось создать окно: {err}",
    en: "could not create the window: {err}",
};
pub const WINDOW_BUFFER_FAILED: Phrase = Phrase {
    ru: "не удалось создать буфер окна: {err}",
    en: "could not create the window buffer: {err}",
};
pub const WINDOW_SURFACE_FAILED: Phrase = Phrase {
    ru: "не удалось создать поверхность окна: {err}",
    en: "could not create the window surface: {err}",
};
pub const NO_HWND: Phrase = Phrase { ru: "у окна нет дескриптора Win32", en: "the window has no Win32 handle" };
pub const HIDE_FAILED: Phrase = Phrase {
    ru: "не удалось подтвердить скрытие окон",
    en: "could not confirm that the windows are hidden",
};
pub const MENU_FAILED: Phrase = Phrase {
    ru: "не удалось собрать меню значка: {err}",
    en: "could not build the tray menu: {err}",
};
pub const ICON_CREATE_FAILED: Phrase = Phrase {
    ru: "не удалось создать значок: {err}",
    en: "could not create the icon: {err}",
};
pub const TRAY_CREATE_FAILED: Phrase = Phrase {
    ru: "не удалось создать значок в области уведомлений: {err}",
    en: "could not create the tray icon: {err}",
};
pub const ICON_READ_FAILED: Phrase = Phrase {
    ru: "не удалось прочитать значок: {err}",
    en: "could not read the icon: {err}",
};
pub const ICON_DECODE_FAILED: Phrase = Phrase { ru: "не удалось прочитать значок", en: "could not read the icon" };
pub const TOO_MANY_SHORT: Phrase = Phrase { ru: "лимит окон достигнут", en: "the window limit has been reached" };
pub const FRAME_BUDGET: Phrase = Phrase {
    ru: "кадр {w}x{h} занимает {bytes} байт, уже занято {used} байт",
    en: "frame {w}x{h} uses {bytes} bytes, {used} bytes are already used",
};
pub const BAD_SCALE_SHORT: Phrase = Phrase { ru: "масштаб монитора некорректен", en: "the monitor scale is invalid" };
pub const EVENT_LOOP_CREATE_FAILED: Phrase = Phrase {
    ru: "не удалось создать цикл событий: {err}",
    en: "could not create the event loop: {err}",
};
pub const EVENT_LOOP_RUN_FAILED: Phrase = Phrase {
    ru: "цикл событий завершился с ошибкой: {err}",
    en: "the event loop stopped with an error: {err}",
};
pub const ENUM_MONITORS_FAILED: Phrase = Phrase {
    ru: "не удалось перечислить мониторы: {err}",
    en: "could not list monitors: {err}",
};
pub const MONITOR_NAME_FAILED: Phrase = Phrase {
    ru: "не удалось прочитать имя монитора: {err}",
    en: "could not read the monitor name: {err}",
};
pub const MONITOR_ORIGIN_FAILED: Phrase = Phrase {
    ru: "не удалось прочитать положение монитора {id}: {err}",
    en: "could not read the position of monitor {id}: {err}",
};
pub const MONITOR_SIZE_FAILED: Phrase = Phrase {
    ru: "не удалось прочитать размер монитора {id}: {err}",
    en: "could not read the size of monitor {id}: {err}",
};
pub const MONITOR_SCALE_FAILED: Phrase = Phrase {
    ru: "не удалось прочитать масштаб монитора {id}: {err}",
    en: "could not read the scale of monitor {id}: {err}",
};
pub const NEGATIVE_CAPTURE: Phrase = Phrase {
    ru: "прямоугольник захвата выходит в отрицательные координаты монитора",
    en: "the capture rectangle extends into negative monitor coordinates",
};
pub const FRAME_TOO_BIG: Phrase = Phrase {
    ru: "размер кадра превышает 64 МиБ или переполняет расчёт буфера",
    en: "the frame is larger than 64 MiB or overflows the buffer calculation",
};
pub const MONITOR_NOT_FOUND: Phrase = Phrase {
    ru: "монитор {id} не найден",
    en: "monitor {id} was not found",
};
pub const WORK_AREA_SCALE: Phrase = Phrase {
    ru: "масштаб монитора не позволяет перевести рабочую область",
    en: "the monitor scale cannot convert the work area",
};
pub const SAVE_DIALOG_FAILED: Phrase = Phrase {
    ru: "диалог сохранения вернул ошибку {code}",
    en: "the save dialog returned error {code}",
};
pub const FRAME_BUFFER_MISMATCH: Phrase = Phrase {
    ru: "буфер кадра не совпадает с шириной и высотой",
    en: "the frame buffer does not match the width and height",
};
