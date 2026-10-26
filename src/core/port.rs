use std::path::Path;
use std::time::SystemTime;

use crate::core::geometry::CaptureRect;
use crate::core::language::Language;
use crate::core::refresh::Fingerprint;

pub trait PlatformPort {
    type Window: Clone;

    fn now(&self) -> SystemTime;
    fn system_language(&self) -> Language;
    fn read_language(&self) -> Result<Option<Language>, String>;
    fn write_language(&mut self, language: Language) -> Result<(), String>;
    fn monitors(&self) -> Result<Vec<Fingerprint>, String>;
    fn snapshot(&mut self, hide: &[Self::Window]) -> Result<Vec<(Fingerprint, Vec<u8>)>, String>;
    fn capture_region(
        &mut self,
        monitor_id: &str,
        rect: &CaptureRect,
        hide: &[Self::Window],
    ) -> Result<(u32, u32, Vec<u8>), String>;
    fn write_png(&mut self, path: &Path, pixels: &[u8], width: u32, height: u32) -> Result<(), String>;
    fn copy_image(&mut self, pixels: &[u8], width: u32, height: u32) -> Result<(), String>;
}
