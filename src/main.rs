mod audio;
mod autostart;
mod editor;
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

/// Messages from the tokio side to the eframe UI.
#[derive(Debug)]
pub enum UiMessage {
    Fire(UiFire),
    Snapshot(Vec<parser::Reminder>),
    LastFired {
        schedule: String,
        at: DateTime<Local>,
    },
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
    runtime_handle.spawn(async move {
        if let Err(e) = run_async(
            runtime_for_async,
            ui_tx,
            reload_rx,
            reload_tx_for_watcher,
            paused_for_async,
        )
        .await
        {
            warn!(error = %e, "async lifecycle exited with error");
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("kree")
            .with_inner_size([main_window::WIDTH, main_window::HEIGHT])
            .with_visible(false),
        ..Default::default()
    };

    let run_result = eframe::run_native(
        "kree",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(App::new(ui_rx, reload_tx, paused)?))
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
    let path = paths::reminders_path()?;

    // Watcher kept alive for the lifetime of this task.
    let _watcher = match watcher::watch(&path, reload_tx_for_watcher) {
        Ok(w) => Some(w),
        Err(e) => {
            warn!(error = %e, "file watcher unavailable; hot reload disabled");
            None
        }
    };

    let mut current = load_reminders(&path);
    let _ = ui_tx.send(UiMessage::Snapshot(current.clone()));
    info!(count = current.len(), "scheduling reminders");

    let (sched_tx, mut sched_rx) = mpsc::channel::<ReminderEvent>(64);
    let mut scheduler = Scheduler::spawn(current.clone(), sched_tx.clone());

    loop {
        tokio::select! {
            Some(event) = sched_rx.recv() => {
                if paused.load(Ordering::Acquire) {
                    info!(schedule = %event.schedule, "skipping fire (paused)");
                    continue;
                }
                handle_fire(&runtime, &ui_tx, event);
            }
            Some(()) = reload_rx.recv() => {
                info!("reloading reminders");
                drop(scheduler);
                current = load_reminders(&path);
                let _ = ui_tx.send(UiMessage::Snapshot(current.clone()));
                info!(count = current.len(), "rescheduling reminders");
                scheduler = Scheduler::spawn(current.clone(), sched_tx.clone());
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
) {
    info!(
        schedule = %event.schedule,
        icon = %event.icon,
        body = %event.body,
        fired_at = %event.fired_at,
        "reminder fired"
    );
    audio::play_chime();

    let visible = Arc::new(AtomicBool::new(true));

    let body_for_tts = event.body.clone();
    let visible_for_tts = Arc::clone(&visible);
    runtime.spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if visible_for_tts.load(Ordering::Acquire) {
            audio::speak(body_for_tts);
        }
    });

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

/// Open the most recent log file in the OS default handler (typically
/// Notepad on Windows). Falls back to opening the logs directory when
/// nothing has been written yet.
fn open_log() -> Result<()> {
    let target = match paths::latest_log_file()? {
        Some(p) => p,
        None => paths::logs_dir()?,
    };
    info!(path = %target.display(), "opening log target");
    std::process::Command::new("cmd")
        .args(["/C", "start", "", target.to_str().unwrap_or("")])
        .spawn()
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("spawning shell open: {e}"))
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
    quitting: bool,
    window_state: main_window::MainWindowState,
    reminders: Vec<parser::Reminder>,
    last_fired: HashMap<String, DateTime<Local>>,
}

impl App {
    fn new(
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
        Ok(Self {
            tray,
            ui_rx,
            reload_tx,
            paused,
            popups: Vec::new(),
            main_visible: false,
            quitting: false,
            window_state,
            reminders: Vec::new(),
            last_fired: HashMap::new(),
        })
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
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quitting {
                // Let it close.
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.main_visible = false;
            }
        }

        // Drain UI messages from the tokio side.
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMessage::Fire(fire) => {
                    let anchor = self.tray.rect_anchor();
                    let position = popup::compute_position(&self.popups, anchor);
                    info!(
                        icon = %fire.icon,
                        body = %fire.body,
                        position = ?position,
                        "opening popup"
                    );
                    self.popups.push(popup::PopupHandle::new(fire, position));
                }
                UiMessage::Snapshot(reminders) => {
                    info!(count = reminders.len(), "main window snapshot updated");
                    self.reminders = reminders;
                }
                UiMessage::LastFired { schedule, at } => {
                    self.last_fired.insert(schedule, at);
                }
            }
        }

        // Drain tray menu events.
        let menu_rx = MenuEvent::receiver();
        while let Ok(menu_event) = menu_rx.try_recv() {
            let id = menu_event.id();
            if id == &self.tray.quit_id {
                info!("Quit menu clicked; closing root viewport");
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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

        // Tray left-click toggles the main window.
        let tray_rx = TrayIconEvent::receiver();
        while let Ok(tray_event) = tray_rx.try_recv() {
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
            }
        }

        if self.main_visible {
            let actions = main_window::render(
                ui,
                &mut self.window_state,
                &self.reminders,
                &self.last_fired,
            );
            for action in actions {
                self.handle_main_window_action(action);
            }
        }

        self.popups.retain_mut(|popup| popup.show(&ctx));

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
