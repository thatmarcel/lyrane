use crate::app_window::AppWindow;
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{msg_send, ClassType, MainThreadOnly};
use objc2_app_kit::{NSScreen, NSTextAlignment, NSTextField, NSVisualEffectView};
use objc2_foundation::{NSArray, NSFileManager, NSPoint, NSRect, NSSearchPathDirectory, NSSearchPathDomainMask, NSSize, NSURL};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn app_data_directory_path() -> Option<PathBuf> {
    unsafe {
        let file_manager = NSFileManager::defaultManager();
        let app_support_dirs: Retained<NSArray<NSURL>> = msg_send![
            &file_manager,
            URLsForDirectory: NSSearchPathDirectory::ApplicationSupportDirectory,
            inDomains: NSSearchPathDomainMask::UserDomainMask
        ];

        let Some(home_directory_url) = app_support_dirs.firstObject() else { return None; };
        let Some(home_directory_path_ns_string) = home_directory_url.path() else { return None; };
        let home_directory_path = PathBuf::from(home_directory_path_ns_string.to_string());

        let app_data_directory_path = home_directory_path.join("Lyrane");
        let _ = std::fs::create_dir_all(&app_data_directory_path);

        Some(app_data_directory_path)
    }
}

fn lock_file_path() -> Option<PathBuf> {
    app_data_directory_path().map(|path| path.join("lock-file"))
}

pub fn create_lock_file() -> Option<File> {
    lock_file_path().and_then(|path| {
        let Ok(mut file) = File::create(&path) else { return None; };

        let Ok(_) = file.try_lock() else { return None };

        file.write(&[]).expect("Failed to write lock file");

        Some(file)
    })
}

pub fn remove_lock_file(lock_file: &File) {
    let _ = lock_file.unlock();
    lock_file_path().and_then(|path| std::fs::remove_file(path).ok());
}

fn config_file_path() -> Option<PathBuf> {
    app_data_directory_path().map(|path| path.join("config.json"))
}

pub fn store_persistent_config(config: &PersistentStoredConfig) {
    let Some(config_file_path) = config_file_path() else { return; };
    let Ok(config_file) = File::create(config_file_path) else { return; };

    let _ = serde_json::to_writer(config_file, &config);
}

pub fn store_persistent_config_from_app_window(app_window: &AppWindow) {
    let Some(main_screen) = NSScreen::mainScreen(app_window.mtm()) else { return; };
    let Some(app_window_content_view) = app_window.contentView() else { return; };
    let Some(lyrics_line_text_view)= app_window_content_view.subviews()
        .iter()
        .find(|subview| subview.isKindOfClass(NSTextField::class()))
        .and_then(|view| view.downcast::<NSTextField>().ok())
    else { return; };

    store_persistent_config(
        &PersistentStoredConfig {
            previous_screen_size: main_screen.frame().size.into(),
            previous_window_frame: app_window.frame().into(),
            previous_text_alignment: lyrics_line_text_view.alignment().into(),
            previously_had_background_enabled: app_window_content_view
                .subviews()
                .iter()
                .any(|subview| subview.isKindOfClass(NSVisualEffectView::class()) && !subview.isHidden())
        }
    );
}

pub fn load_persistent_config() -> Option<PersistentStoredConfig> {
    let Some(config_file_path) = config_file_path() else { return None; };
    let Ok(config_file) = File::open(&config_file_path) else { return None };

    serde_json::from_reader(config_file).ok()
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct PersistentStoredConfig {
    pub previous_screen_size: StoredSize,
    pub previous_window_frame: StoredRect,
    pub previous_text_alignment: StoredTextAlignment,
    pub previously_had_background_enabled: bool
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct StoredRect {
    pub origin: StoredPoint,
    pub size: StoredSize
}

impl From<NSRect> for StoredRect {
    fn from(value: NSRect) -> Self {
        Self {
            origin: StoredPoint::from(value.origin),
            size: StoredSize::from(value.size)
        }
    }
}

impl From<StoredRect> for NSRect {
    fn from(value: StoredRect) -> Self {
        Self {
            origin: NSPoint::from(value.origin),
            size: NSSize::from(value.size)
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct StoredPoint {
    pub x: f64,
    pub y: f64
}

impl From<NSPoint> for StoredPoint {
    fn from(value: NSPoint) -> Self {
        Self {
            x: value.x,
            y: value.y
        }
    }
}

impl From<StoredPoint> for NSPoint {
    fn from(value: StoredPoint) -> Self {
        Self {
            x: value.x,
            y: value.y
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct StoredSize {
    pub width: f64,
    pub height: f64
}

impl From<NSSize> for StoredSize {
    fn from(value: NSSize) -> Self {
        Self {
            width: value.width,
            height: value.height
        }
    }
}

impl From<StoredSize> for NSSize {
    fn from(value: StoredSize) -> Self {
        Self {
            width: value.width,
            height: value.height
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum StoredTextAlignment {
    Left,
    Center,
    Right
}

impl From<NSTextAlignment> for StoredTextAlignment {
    fn from(value: NSTextAlignment) -> Self {
        match value {
            NSTextAlignment::Left => StoredTextAlignment::Left,
            NSTextAlignment::Center => StoredTextAlignment::Center,
            NSTextAlignment::Right => StoredTextAlignment::Right,
            _ => StoredTextAlignment::Center
        }
    }
}

impl From<StoredTextAlignment> for NSTextAlignment {
    fn from(value: StoredTextAlignment) -> Self {
        match value {
            StoredTextAlignment::Left => NSTextAlignment::Left,
            StoredTextAlignment::Center => NSTextAlignment::Center,
            StoredTextAlignment::Right => NSTextAlignment::Right
        }
    }
}
