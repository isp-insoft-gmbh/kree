// Release builds run as a Windows GUI subsystem binary — no console
// attaches, so the autostart entry doesn't bring up a `cmd` window
// (and closing that cmd no longer kills kree). Debug builds keep the
// default console subsystem so `cargo run` shows stderr.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
use tray_icon::menu::MenuId;
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

use kree::scheduler::{ReminderEvent, Scheduler};
use kree::theme::ThemeMode;
use kree::{
    UiFire, audio, autostart, config, editor, logging, main_window, parser, paths, popup, theme,
    tray, watcher,
};

const STALE_FIRE_WINDOW: Duration = Duration::from_secs(90);

/// Messages from the tokio side to the eframe UI.
#[derive(Debug)]
pub enum UiMessage {
    Fire(UiFire),
    Snapshot(SnapshotPayload),
    LastFired {
        schedule: String,
        at: DateTime<Local>,
    },
    Shutdown,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMenuAction {
    Quit,
    EditReminders,
    OpenLog,
    None,
}

fn main() -> Result<()> {
    let _log_guard = logging::init()?;
    attach_parent_console_for_ctrl_c();

    let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".into());
    let mutex_name = format!("kree-singleton-{user}");

    let _instance = match SingleInstance::new(&mutex_name)? {
        i if i.is_single() => i,
        _ => {
            warn!("another kree instance is already running; exiting");
            return Ok(());
        }
    };

    let silent = std::env::args().any(|a| a == "--silent");
    info!(silent, "kree starting (user={user})");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_time()
        .build()?;
    let runtime_handle = runtime.handle().clone();

    let (ui_tx, ui_rx) = mpsc::unbounded_channel::<UiMessage>();
    let (reload_tx, reload_rx) = mpsc::unbounded_channel::<()>();
    let paused = Arc::new(AtomicBool::new(false));

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
            theme::install_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx, ThemeMode::Dark);
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
            Ok(Box::new(App::new(
                cc.egui_ctx.clone(),
                runtime_handle.clone(),
                ui_tx.clone(),
                ui_rx,
                reload_tx,
                reload_rx,
                paused,
            )?))
        }),
    );

    info!("eframe loop exited; shutting down runtime");
    runtime.shutdown_timeout(Duration::from_secs(2));
    run_result.map_err(|e| anyhow::anyhow!("eframe failed: {e}"))?;
    Ok(())
}

#[cfg(windows)]
fn attach_parent_console_for_ctrl_c() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, GetConsoleWindow};

    if !unsafe { GetConsoleWindow() }.is_invalid() {
        return;
    }

    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_ok() {
        info!("attached to parent console for Ctrl+C handling");
    }
}

#[cfg(not(windows))]
fn attach_parent_console_for_ctrl_c() {}

async fn run_async(
    runtime: tokio::runtime::Handle,
    ctx: egui::Context,
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
    send_ui(
        &ui_tx,
        &ctx,
        UiMessage::Snapshot(SnapshotPayload {
            reminders: current_reminders.clone(),
            config: current_config.clone(),
        }),
    );
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
                handle_fire(&runtime, &ctx, &ui_tx, event, &current_config);
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

                send_ui(
                    &ui_tx,
                    &ctx,
                    UiMessage::Snapshot(SnapshotPayload {
                        reminders: current_reminders.clone(),
                        config: current_config.clone(),
                    }),
                );
            }
            else => break,
        }
    }
    Ok(())
}

fn handle_fire(
    runtime: &tokio::runtime::Handle,
    ctx: &egui::Context,
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

    send_ui(
        ui_tx,
        ctx,
        UiMessage::LastFired {
            schedule: event.schedule.clone(),
            at: event.fired_at,
        },
    );

    let fire = UiFire {
        icon: event.icon,
        body: event.body,
        fired_at: event.fired_at,
        visible,
    };
    send_ui(ui_tx, ctx, UiMessage::Fire(fire));
}

fn send_ui(
    ui_tx: &mpsc::UnboundedSender<UiMessage>,
    ctx: &egui::Context,
    message: UiMessage,
) -> bool {
    let sent = ui_tx.send(message).is_ok();
    if sent {
        ctx.request_repaint();
    }
    sent
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

fn classify_tray_menu_id(
    id: &MenuId,
    quit_id: &MenuId,
    edit_id: &MenuId,
    log_id: &MenuId,
) -> TrayMenuAction {
    if id == quit_id {
        TrayMenuAction::Quit
    } else if id == edit_id {
        TrayMenuAction::EditReminders
    } else if id == log_id {
        TrayMenuAction::OpenLog
    } else {
        TrayMenuAction::None
    }
}

async fn watch_system_theme(ctx: egui::Context, ui_tx: mpsc::UnboundedSender<UiMessage>) {
    let mut last = read_apps_use_light_theme();
    // Push the initial value so any non-default consumers can sync up.
    send_ui(&ui_tx, &ctx, UiMessage::SystemThemeChanged(last));
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.tick().await; // first tick fires immediately — discard
    loop {
        tick.tick().await;
        let now = read_apps_use_light_theme();
        if now != last {
            last = now;
            if !send_ui(&ui_tx, &ctx, UiMessage::SystemThemeChanged(now)) {
                // UI gone; bail out so the task can drop.
                return;
            }
        }
    }
}

async fn watch_ctrl_c(ctx: egui::Context, ui_tx: mpsc::UnboundedSender<UiMessage>) {
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            info!("Ctrl+C received; requesting shutdown");
            send_ui(&ui_tx, &ctx, UiMessage::Shutdown);
        }
        Err(error) => {
            warn!(%error, "Ctrl+C handler failed");
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
    applied_theme: Option<ThemeMode>,
    shutting_down: bool,
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
        runtime_handle: tokio::runtime::Handle,
        ui_tx: mpsc::UnboundedSender<UiMessage>,
        ui_rx: mpsc::UnboundedReceiver<UiMessage>,
        reload_tx: mpsc::UnboundedSender<()>,
        reload_rx: mpsc::UnboundedReceiver<()>,
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

        let runtime_for_async = runtime_handle.clone();
        let reload_tx_for_watcher = reload_tx.clone();
        let paused_for_async = Arc::clone(&paused);
        let ui_tx_for_async = ui_tx.clone();
        let ctx_for_async = ctx.clone();
        runtime_handle.spawn(async move {
            if let Err(e) = run_async(
                runtime_for_async,
                ctx_for_async,
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
        runtime_handle.spawn(watch_system_theme(ctx.clone(), ui_tx.clone()));
        runtime_handle.spawn(watch_ctrl_c(ctx.clone(), ui_tx));

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
            applied_theme: None,
            shutting_down: false,
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

    fn request_shutdown(&mut self, ctx: &egui::Context) {
        if self.shutting_down {
            return;
        }
        self.shutting_down = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        ctx.request_repaint();
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
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // eframe's default clear_color reads `visuals` *as-passed* — in
        // practice that's egui's stock dark/light defaults from before
        // our `theme::apply` ran. Pin the swap-chain clear directly to
        // our nugu palettes so a hidden-then-shown light viewport
        // doesn't briefly flash dark.
        let bg = match self.effective_theme() {
            ThemeMode::Dark => egui::Color32::from_rgb(0x24, 0x24, 0x24),
            ThemeMode::Light => egui::Color32::from_rgb(0xdb, 0xdb, 0xdb),
        };
        egui::Rgba::from(bg).to_array()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut mode = self.effective_theme();
        self.apply_theme_if_needed(&ctx, mode);

        if ctx.input(|i| i.viewport().close_requested()) {
            // Window-X always means "hide to tray". Real exit goes
            // through explicit shutdown paths such as tray Quit or Ctrl+C.
            if !self.shutting_down {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.main_visible = false;
            }
        }

        // Drain UI messages from the tokio side.
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMessage::Fire(fire) => {
                    if fire_is_stale(fire.fired_at, Local::now()) {
                        fire.visible.store(false, Ordering::Release);
                        warn!(
                            body = %fire.body,
                            fired_at = %fire.fired_at,
                            "dropping stale reminder fire"
                        );
                        continue;
                    }
                    let anchor = self.tray.rect_anchor();
                    mode = self.effective_theme();
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
                    self.reminders = reminders;
                    self.config = config;
                    mode = self.effective_theme();
                    self.apply_theme_if_needed(&ctx, mode);
                }
                UiMessage::LastFired { schedule, at } => {
                    self.last_fired.insert(schedule, at);
                }
                UiMessage::Shutdown => {
                    self.request_shutdown(&ctx);
                }
                UiMessage::SystemThemeChanged(is_light) => {
                    info!(is_light, "system theme changed");
                    self.system_is_light = is_light;
                    mode = self.effective_theme();
                    self.apply_theme_if_needed(&ctx, mode);
                }
            }
        }

        // Drain tray menu events from our own channel (see App::new).
        while let Ok(menu_event) = self.menu_rx.try_recv() {
            let id = menu_event.id();
            match classify_tray_menu_id(
                id,
                &self.tray.quit_id,
                &self.tray.edit_id,
                &self.tray.log_id,
            ) {
                TrayMenuAction::Quit => {
                    info!("Quit menu clicked; exiting");
                    std::process::exit(0);
                }
                TrayMenuAction::EditReminders => {
                    if let Err(e) = open_editor() {
                        warn!(error = %e, "edit reminders failed");
                    }
                }
                TrayMenuAction::OpenLog => {
                    if let Err(e) = open_log() {
                        warn!(error = %e, "open log failed");
                    }
                }
                TrayMenuAction::None => {}
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

        if self.main_visible {
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
        }

        self.popups.retain_mut(|popup| popup.show(&ctx));

        if !self.popups.is_empty() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}

impl App {
    fn apply_theme_if_needed(&mut self, ctx: &egui::Context, mode: ThemeMode) {
        if self.applied_theme == Some(mode) {
            return;
        }
        theme::apply(ctx, mode);
        self.applied_theme = Some(mode);
    }
}

fn fire_is_stale(fired_at: DateTime<Local>, now: DateTime<Local>) -> bool {
    now.signed_duration_since(fired_at)
        .to_std()
        .is_ok_and(|age| age > STALE_FIRE_WINDOW)
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

    #[test]
    fn tray_menu_ids_route_to_expected_actions() {
        let quit = MenuId::new("quit");
        let edit = MenuId::new("edit");
        let log = MenuId::new("log");
        let unknown = MenuId::new("unknown");

        assert_eq!(
            classify_tray_menu_id(&quit, &quit, &edit, &log),
            TrayMenuAction::Quit
        );
        assert_eq!(
            classify_tray_menu_id(&edit, &quit, &edit, &log),
            TrayMenuAction::EditReminders
        );
        assert_eq!(
            classify_tray_menu_id(&log, &quit, &edit, &log),
            TrayMenuAction::OpenLog
        );
        assert_eq!(
            classify_tray_menu_id(&unknown, &quit, &edit, &log),
            TrayMenuAction::None
        );
    }

    #[test]
    fn recent_fire_is_not_stale() {
        let now = Local::now();
        assert!(!fire_is_stale(now - chrono::Duration::seconds(30), now));
    }

    #[test]
    fn old_fire_is_stale() {
        let now = Local::now();
        assert!(fire_is_stale(now - chrono::Duration::minutes(2), now));
    }
}
