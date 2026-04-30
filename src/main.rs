mod audio;
mod autostart;
mod config;
mod editor;
mod fonts;
mod logging;
mod main_window;
mod parser;
mod paths;
mod popup;
mod scheduler;
mod theme;
mod tray;
mod watcher;

use std::collections::HashMap;
use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Local};
use eframe::egui;
use single_instance::SingleInstance;
use tokio::sync::mpsc;
use tracing::{info, warn};
use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

use crate::scheduler::{ReminderEvent, Scheduler};
use crate::theme::ThemeMode;

/// Messages from the tokio side to the eframe UI.
#[derive(Debug)]
pub enum UiMessage {
    Fire(UiFire),
    Snapshot(SnapshotPayload),
    LastFired {
        schedule: String,
        at: DateTime<Local>,
    },
    /// `true` = the user switched Windows into light mode; `false` = dark.
    /// Polled from the registry on a 1 s tick — we only emit on change.
    SystemThemeChanged(bool),
}

/// Snapshot pushed on startup and on every successful hot-reload.
#[derive(Debug, Clone)]
pub struct SnapshotPayload {
    pub reminders: Vec<parser::Reminder>,
    pub config: config::Config,
}

#[derive(Debug)]
pub struct UiFire {
    pub icon: String,
    pub body: String,
    pub fired_at: DateTime<Local>,
    pub visible: Arc<AtomicBool>,
}

fn main() -> Result<()> {
    let _log_guard = logging::init()?;

    let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".into());
    let mutex_name = format!("kree-singleton-{user}");

    let _instance = match SingleInstance::new(&mutex_name)? {
        i if i.is_single() => i,
        _ => {
            warn!("another kree instance is already running; exiting");
            return Ok(());
        }
    };

    info!("kree starting (user={user})");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let runtime_handle = runtime.handle().clone();

    let (ui_tx, ui_rx) = mpsc::unbounded_channel::<UiMessage>();
    let (reload_tx, reload_rx) = mpsc::unbounded_channel::<()>();
    let paused = Arc::new(AtomicBool::new(false));

    let runtime_for_async = runtime_handle.clone();
    let reload_tx_for_watcher = reload_tx.clone();
    let paused_for_async = Arc::clone(&paused);
    let ui_tx_for_async = ui_tx.clone();
    runtime_handle.spawn(async move {
        if let Err(e) = run_async(
            runtime_for_async,
            ui_tx_for_async,
            reload_rx,
            reload_tx_for_watcher,
            paused_for_async,
        )
        .await
        {
            warn!(error = %e, "async lifecycle exited with error");
        }
    });

    // Poll the Windows app-mode registry key once per second and push a
    // SystemThemeChanged on every transition. Aborts cleanly when the
    // runtime is dropped at eframe exit.
    runtime_handle.spawn(watch_system_theme(ui_tx));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("kree")
            .with_inner_size([main_window::WIDTH, main_window::HEIGHT])
            .with_min_inner_size([480.0, 320.0])
            .with_visible(false),
        ..Default::default()
    };

    let run_result = eframe::run_native(
        "kree",
        options,
        Box::new(move |cc| {
            // Initial font install uses default config; the first
            // Snapshot from run_async will trigger a re-install if
            // the user has font.* set.
            theme::install_fonts(&cc.egui_ctx, &config::Config::default());
            theme::apply(&cc.egui_ctx, ThemeMode::Dark);
            Ok(Box::new(App::new(
                cc.egui_ctx.clone(),
                ui_rx,
                reload_tx,
                paused,
            )?))
        }),
    );

    info!("eframe loop exited; shutting down runtime");
    runtime.shutdown_timeout(Duration::from_secs(2));
    run_result.map_err(|e| anyhow::anyhow!("eframe failed: {e}"))?;
    Ok(())
}

async fn run_async(
    runtime: tokio::runtime::Handle,
    ui_tx: mpsc::UnboundedSender<UiMessage>,
    mut reload_rx: mpsc::UnboundedReceiver<()>,
    reload_tx_for_watcher: mpsc::UnboundedSender<()>,
    paused: Arc<AtomicBool>,
) -> Result<()> {
    let reminders_path = paths::reminders_path()?;
    let config_path = paths::config_path()?;
    let data_dir = paths::data_dir()?;
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        warn!(path = %data_dir.display(), error = %e, "could not create data dir");
    }

    // Watch the whole data dir non-recursively so both reminders.txt
    // and config.toml share one debounced reload signal.
    let _watcher = match watcher::watch(&data_dir, reload_tx_for_watcher) {
        Ok(w) => Some(w),
        Err(e) => {
            warn!(error = %e, "file watcher unavailable; hot reload disabled");
            None
        }
    };

    let mut current_reminders = load_reminders(&reminders_path);
    let mut current_config = load_config(&config_path);
    let _ = ui_tx.send(UiMessage::Snapshot(SnapshotPayload {
        reminders: current_reminders.clone(),
        config: current_config.clone(),
    }));
    info!(
        reminders = current_reminders.len(),
        chime = current_config.chime,
        speak = current_config.speak,
        popup_position = current_config.popup_position.as_str(),
        "initial snapshot"
    );

    let (sched_tx, mut sched_rx) = mpsc::channel::<ReminderEvent>(64);
    let mut scheduler = Scheduler::spawn(current_reminders.clone(), sched_tx.clone());

    loop {
        tokio::select! {
            Some(event) = sched_rx.recv() => {
                if paused.load(Ordering::Acquire) {
                    info!(schedule = %event.schedule, "skipping fire (paused)");
                    continue;
                }
                handle_fire(&runtime, &ui_tx, event, &current_config);
            }
            Some(()) = reload_rx.recv() => {
                let new_reminders = load_reminders(&reminders_path);
                let new_config = load_config(&config_path);
                let reminders_changed = new_reminders.len() != current_reminders.len()
                    || new_reminders.iter().zip(current_reminders.iter()).any(|(a, b)| {
                        a.schedule != b.schedule || a.icon != b.icon || a.body != b.body
                    });
                let config_changed = new_config != current_config;

                // Feedback-loop guard: when our own atomic write echoes
                // back through the watcher, the parsed contents are
                // identical and we skip the broadcast.
                if !reminders_changed && !config_changed {
                    continue;
                }

                if reminders_changed {
                    info!("reloading reminders");
                    drop(scheduler);
                    current_reminders = new_reminders;
                    info!(count = current_reminders.len(), "rescheduling reminders");
                    scheduler = Scheduler::spawn(current_reminders.clone(), sched_tx.clone());
                }
                if config_changed {
                    info!(
                        chime = new_config.chime,
                        speak = new_config.speak,
                        popup_position = new_config.popup_position.as_str(),
                        "reloading config"
                    );
                    current_config = new_config;
                }

                let _ = ui_tx.send(UiMessage::Snapshot(SnapshotPayload {
                    reminders: current_reminders.clone(),
                    config: current_config.clone(),
                }));
            }
            else => break,
        }
    }
    Ok(())
}

fn handle_fire(
    runtime: &tokio::runtime::Handle,
    ui_tx: &mpsc::UnboundedSender<UiMessage>,
    event: ReminderEvent,
    config: &config::Config,
) {
    info!(
        schedule = %event.schedule,
        icon = %event.icon,
        body = %event.body,
        fired_at = %event.fired_at,
        "reminder fired"
    );

    if config.chime {
        audio::play_chime();
    }

    let visible = Arc::new(AtomicBool::new(true));

    if config.speak {
        // Gate the timer at spawn — when speak is off we skip the
        // task entirely rather than running the timer just to drop
        // the speech at the end.
        let body_for_tts = event.body.clone();
        let visible_for_tts = Arc::clone(&visible);
        runtime.spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if visible_for_tts.load(Ordering::Acquire) {
                audio::speak(body_for_tts);
            }
        });
    }

    let _ = ui_tx.send(UiMessage::LastFired {
        schedule: event.schedule.clone(),
        at: event.fired_at,
    });

    let fire = UiFire {
        icon: event.icon,
        body: event.body,
        fired_at: event.fired_at,
        visible,
    };
    let _ = ui_tx.send(UiMessage::Fire(fire));
}

fn open_editor() -> Result<()> {
    let path = paths::reminders_path()?;
    editor::open(&path)
}

/// Open the most recent log file in the user's `$VISUAL` / `$EDITOR`
/// (same path as Edit Reminders). On Windows `tracing-appender` holds
/// the active file with an exclusive lock — Notepad fails on it, but
/// any modern editor (VS Code, Helix, Neovim, …) opens with shared
/// reads. Fall back to opening the logs directory if no log file
/// exists yet.
fn open_log() -> Result<()> {
    if let Some(path) = paths::latest_log_file()? {
        return editor::open(&path);
    }
    let dir = paths::logs_dir()?;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!(path = %dir.display(), error = %e, "could not create logs dir; opening anyway");
    }
    info!(path = %dir.display(), "no log file yet; opening logs directory in Explorer");
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("spawning explorer: {e}"))
}

fn load_config(path: &std::path::Path) -> config::Config {
    match config::load(path) {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "config load failed; using defaults");
            config::Config::default()
        }
    }
}

/// Read the Windows app-mode registry key. `true` = light, `false` = dark
/// (or missing key). Pure Win32 — no allocations beyond the wide-string
/// path / value buffers.
fn read_apps_use_light_theme() -> bool {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::core::PCWSTR;

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
    let mut data: u32 = 0;
    let mut size: u32 = std::mem::size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value_name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut _ as *mut _),
            Some(&mut size),
        )
    };
    result.is_ok() && decode_apps_use_light_theme(data)
}

/// Pure helper for the `AppsUseLightTheme` value semantics. `1` = light;
/// any other value (including `0` or registry-default missing) is
/// treated as dark. Extracted so the decoder is unit-testable without
/// hitting the registry.
fn decode_apps_use_light_theme(value: u32) -> bool {
    value == 1
}

async fn watch_system_theme(ui_tx: mpsc::UnboundedSender<UiMessage>) {
    let mut last = read_apps_use_light_theme();
    // Push the initial value so any non-default consumers can sync up.
    let _ = ui_tx.send(UiMessage::SystemThemeChanged(last));
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.tick().await; // first tick fires immediately — discard
    loop {
        tick.tick().await;
        let now = read_apps_use_light_theme();
        if now != last {
            last = now;
            if ui_tx.send(UiMessage::SystemThemeChanged(now)).is_err() {
                // UI gone; bail out so the task can drop.
                return;
            }
        }
    }
}

fn load_reminders(path: &std::path::Path) -> Vec<parser::Reminder> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let report = parser::parse(&contents);
            for (line, err) in &report.errors {
                warn!(line, error = %err, "skipping invalid reminder");
            }
            info!(path = %path.display(), count = report.reminders.len(), "loaded reminders");
            report.reminders
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            info!(path = %path.display(), "no reminders file yet — create it to start scheduling");
            Vec::new()
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "reminders read failed");
            Vec::new()
        }
    }
}

struct App {
    tray: tray::Tray,
    ui_rx: mpsc::UnboundedReceiver<UiMessage>,
    reload_tx: mpsc::UnboundedSender<()>,
    paused: Arc<AtomicBool>,
    popups: Vec<popup::PopupHandle>,
    main_visible: bool,
    window_state: main_window::MainWindowState,
    reminders: Vec<parser::Reminder>,
    config: config::Config,
    /// Cached "system is in light mode" — kept up to date by
    /// `UiMessage::SystemThemeChanged` from the registry poll task.
    system_is_light: bool,
    last_fired: HashMap<String, DateTime<Local>>,
    /// Tray menu events are forwarded into this channel by a callback we
    /// install on `MenuEvent::set_event_handler`. The callback also calls
    /// `Context::request_repaint`, which is the only way to wake the
    /// eframe loop while the root viewport is hidden — without it,
    /// `App::ui` never runs and Quit clicks pile up unread.
    menu_rx: mpsc::UnboundedReceiver<MenuEvent>,
    /// Tray icon click events arrive via the same custom-handler trick.
    tray_event_rx: mpsc::UnboundedReceiver<TrayIconEvent>,
}

impl App {
    fn new(
        ctx: egui::Context,
        ui_rx: mpsc::UnboundedReceiver<UiMessage>,
        reload_tx: mpsc::UnboundedSender<()>,
        paused: Arc<AtomicBool>,
    ) -> Result<Self> {
        let tray = tray::build()?;
        let mut window_state = main_window::MainWindowState::default();
        match autostart::is_enabled() {
            Ok(v) => window_state.autostart = v,
            Err(e) => warn!(error = %e, "could not read autostart state; assuming off"),
        }

        // Replace the default tray-icon event sinks with custom ones that
        // (1) forward into our own channels and (2) ping `request_repaint`
        // so the eframe loop wakes up even with the root viewport hidden.
        // The default sinks just enqueue into a global `Receiver`, which
        // we'd never drain because `App::ui` doesn't run while hidden.
        let (menu_tx, menu_rx) = mpsc::unbounded_channel::<MenuEvent>();
        let menu_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = menu_tx.send(event);
            menu_ctx.request_repaint();
        }));

        let (tray_event_tx, tray_event_rx) = mpsc::unbounded_channel::<TrayIconEvent>();
        let tray_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            let _ = tray_event_tx.send(event);
            tray_ctx.request_repaint();
        }));

        Ok(Self {
            tray,
            ui_rx,
            reload_tx,
            paused,
            popups: Vec::new(),
            main_visible: false,
            window_state,
            reminders: Vec::new(),
            config: config::Config::default(),
            system_is_light: read_apps_use_light_theme(),
            last_fired: HashMap::new(),
            menu_rx,
            tray_event_rx,
        })
    }

    fn effective_theme(&self) -> ThemeMode {
        match self.config.theme {
            config::ThemeChoice::Dark => ThemeMode::Dark,
            config::ThemeChoice::Light => ThemeMode::Light,
            config::ThemeChoice::System => {
                if self.system_is_light {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                }
            }
        }
    }

    fn handle_main_window_action(&mut self, action: main_window::MainWindowAction) {
        match action {
            main_window::MainWindowAction::EditReminders => {
                if let Err(e) = open_editor() {
                    warn!(error = %e, "edit reminders failed");
                }
            }
            main_window::MainWindowAction::Reload => {
                let _ = self.reload_tx.send(());
            }
            main_window::MainWindowAction::SetPaused(p) => {
                self.paused.store(p, Ordering::Release);
                if let Err(e) = self.tray.set_paused(p) {
                    warn!(error = %e, "tray set_paused failed");
                }
                info!(paused = p, "pause state updated");
            }
            main_window::MainWindowAction::SetAutostart(enabled) => {
                match autostart::set(enabled) {
                    Ok(()) => info!(enabled, "autostart updated"),
                    Err(e) => {
                        warn!(error = %e, enabled, "autostart update failed");
                        // Keep the checkbox in sync with reality.
                        match autostart::is_enabled() {
                            Ok(v) => self.window_state.autostart = v,
                            Err(e) => warn!(error = %e, "re-querying autostart failed"),
                        }
                    }
                }
            }
            main_window::MainWindowAction::SetConfig(patch) => {
                // Optimistic local update so the UI reflects the change
                // immediately. The watcher echo from the disk write will
                // re-broadcast the same Snapshot via the feedback-loop
                // guard's no-op path.
                apply_patch_local(&mut self.config, patch);

                let path = match paths::config_path() {
                    Ok(p) => p,
                    Err(e) => {
                        warn!(error = %e, "could not resolve config path; skipping write");
                        return;
                    }
                };
                if let Err(e) = config::apply_patch(&path, patch) {
                    warn!(error = %e, ?patch, "config write failed");
                } else {
                    info!(?patch, "config written");
                }
            }
        }
    }
}

fn apply_patch_local(config: &mut config::Config, patch: config::ConfigPatch) {
    match patch {
        config::ConfigPatch::Chime(v) => config.chime = v,
        config::ConfigPatch::Speak(v) => config.speak = v,
        config::ConfigPatch::PopupPosition(p) => config.popup_position = p,
        config::ConfigPatch::Theme(t) => config.theme = t,
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mode = self.effective_theme();
        // Re-assert visuals every frame. eframe's default style can
        // leak through if we only set it in the creator closure, and
        // the user can toggle config.theme at any time. Idempotent.
        theme::apply(&ctx, mode);

        if ctx.input(|i| i.viewport().close_requested()) {
            // Window-X always means "hide to tray". Real exit goes
            // through the Quit menu, which calls process::exit directly.
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.main_visible = false;
        }

        // Drain UI messages from the tokio side.
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMessage::Fire(fire) => {
                    let anchor = self.tray.rect_anchor();
                    let position =
                        popup::compute_position(&self.popups, anchor, self.config.popup_position);
                    info!(
                        icon = %fire.icon,
                        body = %fire.body,
                        position = ?position,
                        anchor = %self.config.popup_position.as_str(),
                        "opening popup"
                    );
                    self.popups
                        .push(popup::PopupHandle::new(fire, position, mode));
                }
                UiMessage::Snapshot(SnapshotPayload { reminders, config }) => {
                    info!(
                        reminders = reminders.len(),
                        chime = config.chime,
                        speak = config.speak,
                        popup_position = config.popup_position.as_str(),
                        theme = config.theme.as_str(),
                        "main window snapshot updated"
                    );
                    let fonts_changed = self.config.font != config.font;
                    self.reminders = reminders;
                    self.config = config;
                    if fonts_changed {
                        info!("font config changed; rebuilding font atlas");
                        theme::install_fonts(&ctx, &self.config);
                    }
                }
                UiMessage::LastFired { schedule, at } => {
                    self.last_fired.insert(schedule, at);
                }
                UiMessage::SystemThemeChanged(is_light) => {
                    info!(is_light, "system theme changed");
                    self.system_is_light = is_light;
                }
            }
        }

        // Drain tray menu events from our own channel (see App::new).
        while let Ok(menu_event) = self.menu_rx.try_recv() {
            let id = menu_event.id();
            if id == &self.tray.quit_id {
                info!("Quit menu clicked; exiting");
                // The hidden root viewport doesn't reliably honor
                // `ViewportCommand::Close` (eframe stays alive driving
                // popups + tray), so go straight to a process exit.
                // Tracing's non-blocking appender may lose its tail —
                // acceptable for an explicit user-initiated quit.
                std::process::exit(0);
            } else if id == &self.tray.edit_id {
                if let Err(e) = open_editor() {
                    warn!(error = %e, "edit reminders failed");
                }
            } else if id == &self.tray.log_id
                && let Err(e) = open_log()
            {
                warn!(error = %e, "open log failed");
            }
        }

        // Drain tray icon click events. Left-click toggles the main window.
        while let Ok(tray_event) = self.tray_event_rx.try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = tray_event
            {
                self.main_visible = !self.main_visible;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.main_visible));
                if self.main_visible {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                ctx.request_repaint();
            }
        }

        // Always render main-window contents into egui's frame buffer,
        // even when the window is hidden. Otherwise a freshly-shown
        // window flashes empty for one frame before the content lands.
        let actions = main_window::render(
            ui,
            &mut self.window_state,
            &self.reminders,
            &self.config,
            &self.last_fired,
            mode,
        );
        for action in actions {
            self.handle_main_window_action(action);
        }

        self.popups.retain_mut(|popup| popup.show(&ctx));

        // egui only repaints on input by default; we drive it so try_recv
        // keeps draining, 30s auto-dismiss fires on time, and Quit clicks
        // surface within one tick. 33 ms ≈ 30 fps — plenty for our use,
        // and "Quit takes a frame" no longer feels sluggish.
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}

#[cfg(test)]
mod theme_decoder_tests {
    use super::*;

    #[test]
    fn one_means_light() {
        assert!(decode_apps_use_light_theme(1));
    }

    #[test]
    fn zero_means_dark() {
        assert!(!decode_apps_use_light_theme(0));
    }

    #[test]
    fn anything_else_means_dark() {
        // Any non-1 value falls back to dark — guards against a
        // registry value somehow getting set to a stray byte.
        for v in [2u32, 42, u32::MAX] {
            assert!(!decode_apps_use_light_theme(v), "value {v} should be dark");
        }
    }
}
