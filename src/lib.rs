pub mod audio;
pub mod autostart;
pub mod config;
pub mod cron;
pub mod editor;
pub mod logging;
pub mod main_window;
pub mod parser;
pub mod paths;
pub mod popup;
pub mod scheduler;
pub mod theme;
pub mod tray;
pub mod watcher;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use chrono::{DateTime, Local};

#[derive(Debug)]
pub struct UiFire {
    pub icon: String,
    pub body: String,
    pub fired_at: DateTime<Local>,
    pub visible: Arc<AtomicBool>,
}
