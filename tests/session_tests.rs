use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use screenpatch::core::geometry::CaptureRect;
use screenpatch::core::hotkeys::{format_hotkeys, Chord, HotkeyAction, Hotkeys, KeyName};
use screenpatch::core::language::{
    render, text, Language, AUTO_COPY_READ_FAILED, AUTO_COPY_SAVE_FAILED, COPY_FAILED, FRAME_TOO_BIG, GEOMETRY_CHANGED, HOTKEY_READ_FAILED, HOTKEY_WRITE_FAILED,
    LANGUAGE_SAVE_FAILED, SOURCE_UNSUPPORTED, WINDOW_LIMIT,
};
use screenpatch::core::model::CaptureSource;
use screenpatch::core::port::PlatformPort;
use screenpatch::core::refresh::Fingerprint;
use screenpatch::core::session::{Outcome, Request, Session};

struct Fake {
    now: SystemTime,
    language: Language,
    saved: Option<Language>,
    write_error: Option<String>,
    monitors: Vec<Fingerprint>,
    region_frame: Vec<u8>,
    copied: Option<Vec<u8>>,
    saved_png: Option<Vec<u8>>,
    snapshots: u32,
    region_captures: u32,
    hotkey_file: Option<String>,
    hotkey_read_error: Option<String>,
    hotkey_write_error: Option<String>,
    written_hotkeys: Option<String>,
    autocopy_file: Option<String>,
    autocopy_read_error: Option<String>,
    autocopy_write_error: Option<String>,
    written_autocopy: Option<String>,
    copy_error: Option<String>,
}

impl PlatformPort for Fake {
    type Window = u64;

    fn now(&self) -> SystemTime {
        self.now
    }

    fn system_language(&self) -> Language {
        self.language
    }

    fn read_language(&self) -> Result<Option<Language>, String> {
        Ok(self.saved)
    }

    fn write_language(&mut self, language: Language) -> Result<(), String> {
        if let Some(err) = self.write_error.clone() {
            return Err(err);
        }
        self.saved = Some(language);
        Ok(())
    }

    fn monitors(&self) -> Result<Vec<Fingerprint>, String> {
        Ok(self.monitors.clone())
    }

    fn snapshot(&mut self, _hide: &[u64]) -> Result<Vec<(Fingerprint, Vec<u8>)>, String> {
        self.snapshots += 1;
        Ok(self
            .monitors
            .iter()
            .cloned()
            .map(|monitor| (monitor, self.region_frame.clone()))
            .collect())
    }

    fn capture_region(
        &mut self,
        _monitor_id: &str,
        rect: &CaptureRect,
        _hide: &[u64],
    ) -> Result<(u32, u32, Vec<u8>), String> {
        self.region_captures += 1;
        Ok((rect.width, rect.height, self.region_frame.clone()))
    }

    fn write_png(&mut self, _path: &Path, pixels: &[u8], _width: u32, _height: u32) -> Result<(), String> {
        self.saved_png = Some(pixels.to_vec());
        Ok(())
    }

    fn copy_image(&mut self, pixels: &[u8], _width: u32, _height: u32) -> Result<(), String> {
        if let Some(err) = &self.copy_error {
            return Err(err.clone());
        }
        self.copied = Some(pixels.to_vec());
        Ok(())
    }

    fn read_autocopy(&self) -> Result<Option<String>, String> {
        if let Some(err) = &self.autocopy_read_error {
            return Err(err.clone());
        }
        Ok(self.autocopy_file.clone())
    }

    fn write_autocopy(&mut self, text: &str) -> Result<(), String> {
        if let Some(err) = &self.autocopy_write_error {
            return Err(err.clone());
        }
        self.written_autocopy = Some(text.to_string());
        Ok(())
    }

    fn read_hotkeys(&self) -> Result<Option<String>, String> {
        if let Some(err) = &self.hotkey_read_error {
            return Err(err.clone());
        }
        Ok(self.hotkey_file.clone())
    }

    fn write_hotkeys(&mut self, text: &str) -> Result<(), String> {
        if let Some(err) = &self.hotkey_write_error {
            return Err(err.clone());
        }
        self.written_hotkeys = Some(text.to_string());
        Ok(())
    }
}

fn fingerprint() -> Fingerprint {
    Fingerprint {
        id: "m".to_string(),
        origin_x: 0,
        origin_y: 0,
        physical_width: 100,
        physical_height: 100,
        scale: 1.0,
    }
}

fn region() -> CaptureSource {
    CaptureSource::Region {
        rect: CaptureRect { x: 0, y: 0, width: 2, height: 2 },
        fingerprint: fingerprint(),
    }
}

fn pixels() -> Vec<u8> {
    vec![1, 2, 3, 4].repeat(4)
}

fn fresh() -> Vec<u8> {
    vec![9, 9, 9, 9].repeat(4)
}

fn fake_ru() -> Fake {
    Fake {
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(50),
        language: Language::Ru,
        saved: Some(Language::Ru),
        write_error: None,
        monitors: vec![fingerprint()],
        region_frame: pixels(),
        copied: None,
        saved_png: None,
        snapshots: 0,
        region_captures: 0,
        hotkey_file: None,
        hotkey_read_error: None,
        hotkey_write_error: None,
        written_hotkeys: None,
        autocopy_file: None,
        autocopy_read_error: None,
        autocopy_write_error: None,
        written_autocopy: None,
        copy_error: None,
    }
}

fn capture_new(source: CaptureSource, pixels: Vec<u8>) -> Request<u64> {
    Request::CaptureNew {
        source,
        x: 10,
        y: 20,
        width: 30,
        height: 40,
        pixels,
    }
}

fn open_ru(fake: &Fake) -> Session<Fake> {
    Session::open(fake).0
}

#[test]
fn eleventh_capture_and_bad_buffer_are_rejected() {
    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    for _ in 0..10 {
        let outcome = session.handle(&mut fake, capture_new(region(), pixels()));
        assert!(matches!(outcome, Outcome::CaptureSucceeded { number, copy_error: None } if number > 0));
    }
    let rejected = session.handle(&mut fake, capture_new(region(), pixels()));
    let Outcome::CaptureFailed { message } = rejected else {
        panic!("expected the window limit");
    };
    assert!(message.contains(text(Language::Ru, WINDOW_LIMIT)));
    assert_eq!(session.len(), 10);

    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    let rejected = session.handle(&mut fake, capture_new(region(), vec![1, 2, 3]));
    assert!(matches!(
        rejected,
        Outcome::CaptureFailed { message } if message == text(Language::Ru, FRAME_TOO_BIG)
    ));
    assert_eq!(session.len(), 0);
}

#[test]
fn refresh_moves_the_old_frame_to_previous_and_stamps_port_time() {
    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    session.handle(&mut fake, capture_new(region(), pixels()));
    fake.region_frame = fresh();
    let outcome = session.handle(&mut fake, Request::CaptureRefresh { number: 1, hide: vec![7] });
    assert!(matches!(outcome, Outcome::CaptureSucceeded { number: 1, copy_error: None }));
    let capture = session.capture(1).expect("capture");
    let previous = capture.previous.as_ref().expect("previous frame");
    assert_eq!(previous.pixels.as_ref(), pixels().as_slice());
    assert_eq!(capture.current.pixels.as_ref(), fresh().as_slice());
    assert_eq!(capture.current.captured_at, fake.now);
    assert_eq!(fake.region_captures, 1);
}

#[test]
fn missing_monitor_is_source_unavailable_and_geometry_change_fails() {
    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    session.handle(&mut fake, capture_new(region(), pixels()));
    fake.monitors.clear();
    let outcome = session.handle(&mut fake, Request::CaptureRefresh { number: 1, hide: vec![] });
    assert!(matches!(outcome, Outcome::SourceUnavailable { .. }));
    assert_eq!(fake.region_captures, 0);
    assert_eq!(session.capture(1).unwrap().current.pixels.as_ref(), pixels().as_slice());

    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    session.handle(&mut fake, capture_new(region(), pixels()));
    fake.monitors[0].physical_width = 50;
    let outcome = session.handle(&mut fake, Request::CaptureRefresh { number: 1, hide: vec![] });
    assert!(matches!(
        outcome,
        Outcome::CaptureFailed { message } if message == text(Language::Ru, GEOMETRY_CHANGED)
    ));
    assert_eq!(fake.region_captures, 0);
}

#[test]
fn window_and_import_do_not_call_the_port() {
    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    for source in [CaptureSource::Window, CaptureSource::Import] {
        let outcome = session.handle(&mut fake, capture_new(source, pixels()));
        assert!(matches!(
            outcome,
            Outcome::CaptureFailed { message } if message == text(Language::Ru, SOURCE_UNSUPPORTED)
        ));
    }
    assert_eq!(fake.snapshots, 0);
    assert_eq!(fake.region_captures, 0);
}

#[test]
fn copy_and_save_send_the_shown_frame() {
    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    session.handle(&mut fake, capture_new(region(), pixels()));
    fake.region_frame = fresh();
    session.handle(&mut fake, Request::CaptureRefresh { number: 1, hide: vec![] });
    assert!(matches!(
        session.handle(&mut fake, Request::ShownToggled { number: 1 }),
        Outcome::ViewUpdated
    ));
    assert!(matches!(session.handle(&mut fake, Request::Copy { number: 1 }), Outcome::CopySucceeded));
    assert!(matches!(
        session.handle(&mut fake, Request::Save { number: 1, path: PathBuf::from("a.png") }),
        Outcome::SaveSucceeded
    ));
    assert_eq!(fake.copied.as_deref(), Some(pixels().as_slice()));
    assert_eq!(fake.saved_png.as_deref(), Some(pixels().as_slice()));
}

#[test]
fn window_moved_changes_only_the_view() {
    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    session.handle(&mut fake, capture_new(region(), pixels()));
    let outcome = session.handle(
        &mut fake,
        Request::WindowMoved { number: 1, x: 4, y: 5, width: 6, height: 7 },
    );
    assert!(matches!(outcome, Outcome::ViewUpdated));
    let capture = session.capture(1).unwrap();
    assert_eq!(capture.view.x, 4);
    assert_eq!(capture.view.y, 5);
    assert_eq!(capture.view.width, 6);
    assert_eq!(capture.view.height, 7);
    assert_eq!(capture.view.opacity, 255);
    assert_eq!(capture.current.pixels.as_ref(), pixels().as_slice());
    assert!(matches!(
        session.handle(
            &mut fake,
            Request::WindowMoved { number: 99, x: 1, y: 1, width: 1, height: 1 },
        ),
        Outcome::Ignored
    ));
}

#[test]
fn language_save_keeps_the_new_language_when_the_file_fails() {
    let mut fake = fake_ru();
    fake.write_error = Some("disk".to_string());
    let mut session = open_ru(&fake);
    let outcome = session.handle(&mut fake, Request::LanguageChosen { language: Language::En });
    assert_eq!(session.language(), Language::En);
    let expected = render(Language::En, LANGUAGE_SAVE_FAILED, &[("err", "disk")]);
    assert!(matches!(outcome, Outcome::LanguageSaved { message: Some(message) } if message == expected));
    assert!(fake.copied.is_none());
    assert!(fake.saved_png.is_none());
    assert_eq!(fake.saved, Some(Language::Ru));
}

#[test]
fn opacity_starts_at_255_and_does_not_follow_the_pixels() {
    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    let created = session.handle(&mut fake, capture_new(region(), pixels()));
    assert!(matches!(created, Outcome::CaptureSucceeded { number: 1, copy_error: None }));
    assert_eq!(session.capture(1).unwrap().view.opacity, 255);

    let lowered = session.handle(&mut fake, Request::OpacitySet { number: 1, opacity: 10 });
    assert_eq!(lowered, Outcome::ViewUpdated);
    assert_eq!(session.capture(1).unwrap().view.opacity, 26);

    let unknown = session.handle(&mut fake, Request::OpacitySet { number: 99, opacity: 100 });
    assert_eq!(unknown, Outcome::Ignored);

    fake.region_frame = fresh();
    session.handle(&mut fake, Request::CaptureRefresh { number: 1, hide: vec![] });
    assert_eq!(session.capture(1).unwrap().view.opacity, 26);
    session.handle(&mut fake, Request::Copy { number: 1 });
    assert_eq!(fake.copied.as_deref(), Some(fresh().as_slice()));
    session.handle(&mut fake, Request::Save { number: 1, path: PathBuf::from("shot.png") });
    assert_eq!(fake.saved_png.as_deref(), Some(fresh().as_slice()));
    session.handle(&mut fake, Request::ShownToggled { number: 1 });
    assert_eq!(session.capture(1).unwrap().view.opacity, 26);
}

#[test]
fn lock_starts_open_and_survives_the_frame() {
    let mut fake = fake_ru();
    let mut session = open_ru(&fake);
    let created = session.handle(&mut fake, capture_new(region(), pixels()));
    assert!(matches!(created, Outcome::CaptureSucceeded { number: 1, copy_error: None }));
    assert!(!session.capture(1).unwrap().view.locked);

    let closed = session.handle(&mut fake, Request::LockToggled { number: 1 });
    assert_eq!(closed, Outcome::ViewUpdated);
    assert!(session.capture(1).unwrap().view.locked);

    let unknown = session.handle(&mut fake, Request::LockToggled { number: 99 });
    assert_eq!(unknown, Outcome::Ignored);
    assert!(session.capture(1).unwrap().view.locked);

    session.handle(
        &mut fake,
        Request::OpacitySet { number: 1, opacity: 40 },
    );
    fake.region_frame = fresh();
    session.handle(&mut fake, Request::CaptureRefresh { number: 1, hide: vec![] });
    session.handle(&mut fake, Request::Copy { number: 1 });
    session.handle(
        &mut fake,
        Request::Save { number: 1, path: PathBuf::from("shot.png") },
    );
    session.handle(&mut fake, Request::ShownToggled { number: 1 });
    session.handle(
        &mut fake,
        Request::WindowMoved { number: 1, x: 9, y: 8, width: 7, height: 6 },
    );
    let capture = session.capture(1).unwrap();
    assert!(capture.view.locked);
    assert_eq!(capture.view.opacity, 40);
    assert_eq!(capture.view.x, 9);
    assert_eq!(capture.view.y, 8);
    assert_eq!(capture.view.width, 7);
    assert_eq!(capture.view.height, 6);
    assert_eq!(fake.copied.as_deref(), Some(fresh().as_slice()));

    let opened = session.handle(&mut fake, Request::LockToggled { number: 1 });
    assert_eq!(opened, Outcome::ViewUpdated);
    assert!(!session.capture(1).unwrap().view.locked);
}

#[test]
fn hotkeys_start_from_the_file_and_a_bad_file_keeps_the_defaults() {
    let mut fake = fake_ru();
    fake.hotkey_file = Some("refresh=q\nbad\n".to_string());
    let session = Session::open(&fake).0;
    let mut expected = Hotkeys::default();
    expected.refresh.key = KeyName::Letter('q');
    assert_eq!(session.hotkeys().refresh, expected.refresh);
    assert_eq!(session.hotkeys().capture, Hotkeys::default().capture);

    fake.hotkey_file = None;
    fake.hotkey_read_error = Some("denied".to_string());
    let (session, _, message, _) = Session::open(&fake);
    assert_eq!(session.hotkeys(), &Hotkeys::default());
    assert_eq!(
        message.as_deref(),
        Some(render(Language::Ru, HOTKEY_READ_FAILED, &[("err", "denied")]).as_str())
    );
}

#[test]
fn choosing_a_hotkey_writes_the_file_and_a_duplicate_does_not() {
    let mut fake = fake_ru();
    let mut session = open_ru(&mut fake);
    let outcome = session.handle(
        &mut fake,
        Request::HotkeyChosen {
            action: HotkeyAction::Refresh,
            chord: Chord {
                key: KeyName::Letter('q'),
                ..Hotkeys::default().refresh
            },
        },
    );
    assert_eq!(outcome, Outcome::HotkeySaved);
    assert_eq!(
        fake.written_hotkeys.as_deref(),
        Some(format_hotkeys(session.hotkeys()).as_str())
    );

    let duplicate = session.handle(
        &mut fake,
        Request::HotkeyChosen {
            action: HotkeyAction::Capture,
            chord: Chord {
                ctrl: false,
                alt: false,
                shift: false,
                win: false,
                key: KeyName::Letter('k'),
            },
        },
    );
    assert_eq!(duplicate, Outcome::Ignored);
    assert!(fake.written_hotkeys.as_ref().unwrap().contains("capture=ctrl+shift+s"));

    fake.hotkey_write_error = Some("full".to_string());
    let failed = session.handle(
        &mut fake,
        Request::HotkeyChosen {
            action: HotkeyAction::Again,
            chord: Chord {
                key: KeyName::Letter('w'),
                ..Hotkeys::default().again
            },
        },
    );
    assert_eq!(
        failed,
        Outcome::HotkeyFailed {
            message: render(Language::Ru, HOTKEY_WRITE_FAILED, &[("err", "full")])
        }
    );
    assert_eq!(session.hotkeys().again, Hotkeys::default().again);
}

#[test]
fn autocopy_starts_off_and_a_bad_file_stays_off() {
    let mut fake = fake_ru();
    let session = Session::open(&fake).0;
    assert!(!session.auto_copy());

    fake.autocopy_file = Some("on\n".to_string());
    let session = Session::open(&fake).0;
    assert!(session.auto_copy());

    fake.autocopy_file = Some(" \n".to_string());
    let (session, _, _, message) = Session::open(&fake);
    assert!(!session.auto_copy());
    assert!(message.is_none());

    fake.autocopy_file = None;
    fake.autocopy_read_error = Some("denied".to_string());
    let (session, _, _, message) = Session::open(&fake);
    assert!(!session.auto_copy());
    assert_eq!(
        message.as_deref(),
        Some(render(Language::Ru, AUTO_COPY_READ_FAILED, &[("err", "denied")]).as_str())
    );
}

#[test]
fn choosing_autocopy_writes_the_file_and_keeps_the_choice_when_the_write_fails() {
    let mut fake = fake_ru();
    let mut session = open_ru(&mut fake);
    let outcome = session.handle(&mut fake, Request::AutoCopyChosen { enabled: true });
    assert_eq!(outcome, Outcome::AutoCopySaved { message: None });
    assert!(session.auto_copy());
    assert_eq!(fake.written_autocopy.as_deref(), Some("on\n"));

    fake.autocopy_write_error = Some("full".to_string());
    let failed = session.handle(&mut fake, Request::AutoCopyChosen { enabled: false });
    assert!(!session.auto_copy());
    assert_eq!(
        failed,
        Outcome::AutoCopySaved {
            message: Some(render(Language::Ru, AUTO_COPY_SAVE_FAILED, &[("err", "full")]))
        }
    );
}

#[test]
fn a_new_capture_copies_only_while_autocopy_is_on() {
    let mut fake = fake_ru();
    let mut session = open_ru(&mut fake);
    let created = session.handle(&mut fake, capture_new(region(), pixels()));
    assert_eq!(created, Outcome::CaptureSucceeded { number: 1, copy_error: None });
    assert!(fake.copied.is_none());

    session.handle(&mut fake, Request::AutoCopyChosen { enabled: true });
    fake.copied = None;
    let copied = session.handle(&mut fake, capture_new(region(), pixels()));
    assert_eq!(copied, Outcome::CaptureSucceeded { number: 2, copy_error: None });
    assert_eq!(fake.copied.as_deref(), Some(pixels().as_slice()));

    fake.copied = None;
    let refreshed = session.handle(&mut fake, Request::CaptureRefresh { number: 2, hide: Vec::new() });
    assert_eq!(refreshed, Outcome::CaptureSucceeded { number: 2, copy_error: None });
    assert!(fake.copied.is_none());

    fake.copy_error = Some("busy".to_string());
    let failed = session.handle(&mut fake, capture_new(region(), pixels()));
    assert_eq!(
        failed,
        Outcome::CaptureSucceeded {
            number: 3,
            copy_error: Some(render(
                Language::Ru,
                COPY_FAILED,
                &[("number", "3"), ("err", "busy")]
            ))
        }
    );
    assert!(session.capture(3).is_some());
}
