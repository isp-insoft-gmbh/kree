mod audio;
mod logging;
mod main_window;
mod parser;
mod paths;
mod popup;
mod scheduler;
mod tray;

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

/// One UI-bound fire, surfaced to the eframe app from the tokio side.
#[derive(Debug)]
pub struct UiFire {
    pub icon: String,
    pub body: String,
    pub fired_at: DateTime<Local>,
    pub visible: Arc<AtomicBool>,
}

fn main() -> Result<()> {
    // Hold the guard for the entire process lifetime so the appender flushes on exit.
    let _log_guard = logging::init()?;

    let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".into());
    let mutex_name = format!("kree-singleton-{user}");

    // The mutex must outlive the program; bind it to `_instance` so it isn't dropped early.
    let _instance = match SingleInstance::new(&mutex_name)? {
        i if i.is_single() => i,
        _ => {
            warn!("another kree instance is already running; exiting");
            return Ok(());
        }
    };

    info!("kree starting (user={user})");

    // Tokio runtime lives on a worker thread so the main thread is free
    // for eframe's winit event loop. tray-icon registers its hidden Win32
    // window from inside the eframe creator closure (same thread that
    // pumps messages).
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let runtime_handle = runtime.handle().clone();

    let (ui_tx, ui_rx) = mpsc::unbounded_channel::<UiFire>();

    let runtime_for_async = runtime_handle.clone();
    runtime_handle.spawn(async move {
        if let Err(e) = run_async(runtime_for_async, ui_tx).await {
            warn!(error = %e, "async lifecycle exited with error");
        }
    });

    // Root viewport == the main window (SPEC.md § 5.5). Starts hidden;
    // a tray left-click toggles visibility. Closing the X hides it
    // rather than quitting (handled in `App::ui`).
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
        Box::new(move |_cc| Ok(Box::new(App::new(ui_rx)?))),
    );

    info!("eframe loop exited; shutting down runtime");
    runtime.shutdown_timeout(Duration::from_secs(2));
    run_result.map_err(|e| anyhow::anyhow!("eframe failed: {e}"))?;
    Ok(())
}

async fn run_async(
    runtime: tokio::runtime::Handle,
    ui_tx: mpsc::UnboundedSender<UiFire>,
) -> Result<()> {
    let reminders = load_reminders()?;
    info!(count = reminders.len(), "scheduling reminders");

    let (tx, mut rx) = mpsc::channel::<ReminderEvent>(64);
    // `_sched` keeps the scheduler tasks alive; dropping it would abort them.
    let _sched = Scheduler::spawn(reminders, tx);

    while let Some(event) = rx.recv().await {
        info!(
            schedule = %event.schedule,
            icon = %event.icon,
            body = %event.body,
            fired_at = %event.fired_at,
            "reminder fired"
        );
        audio::play_chime();

        let visible = Arc::new(AtomicBool::new(true));

        // 2-second TTS gate: speak only if the popup is still on screen.
        let body_for_tts = event.body.clone();
        let visible_for_tts = Arc::clone(&visible);
        runtime.spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if visible_for_tts.load(Ordering::Acquire) {
                audio::speak(body_for_tts);
            }
        });

        let ui_event = UiFire {
            icon: event.icon,
            body: event.body,
            fired_at: event.fired_at,
            visible,
        };
        if ui_tx.send(ui_event).is_err() {
            // UI gone; we're shutting down.
            return Ok(());
        }
    }
    Ok(())
}

fn load_reminders() -> Result<Vec<parser::Reminder>> {
    let path = paths::reminders_path()?;
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let report = parser::parse(&contents);
            for (line, err) in &report.errors {
                warn!(line, error = %err, "skipping invalid reminder");
            }
            info!(path = %path.display(), count = report.reminders.len(), "loaded reminders");
            Ok(report.reminders)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            info!(path = %path.display(), "no reminders file yet — create it to start scheduling");
            Ok(Vec::new())
        }
        Err(e) => Err(e.into()),
    }
}

struct App {
    tray: tray::Tray,
    ui_rx: mpsc::UnboundedReceiver<UiFire>,
    popups: Vec<popup::PopupHandle>,
    main_visible: bool,
    quitting: bool,
    window_state: main_window::MainWindowState,
    reminders: Vec<parser::Reminder>,
}

impl App {
    fn new(ui_rx: mpsc::UnboundedReceiver<UiFire>) -> Result<Self> {
        // Tray must be built on the same thread that pumps Win32 messages
        // (the eframe / winit thread). The creator closure runs there.
        let tray = tray::build()?;
        let reminders = load_reminders().unwrap_or_else(|e| {
            warn!(error = %e, "main window: load_reminders failed; starting empty");
            Vec::new()
        });
        Ok(Self {
            tray,
            ui_rx,
            popups: Vec::new(),
            main_visible: false,
            quitting: false,
            window_state: main_window::MainWindowState::default(),
            reminders,
        })
    }

    fn handle_main_window_action(&mut self, action: main_window::MainWindowAction) {
        match action {
            main_window::MainWindowAction::EditReminders => {
                // step 12 will spawn the editor
                info!("Edit Reminders clicked (no-op until step 12)");
            }
            main_window::MainWindowAction::Reload => {
                match load_reminders() {
                    Ok(rs) => {
                        info!(count = rs.len(), "main window reload");
                        self.reminders = rs;
                    }
                    Err(e) => warn!(error = %e, "reload failed"),
                }
                // step 13 will also re-spawn the scheduler
            }
            main_window::MainWindowAction::SetPaused(p) => {
                // step 13/15 will actually pause the scheduler
                info!(paused = p, "Pause toggled (no-op until step 13)");
            }
            main_window::MainWindowAction::SetAutostart(a) => {
                // step 14 will write/remove HKCU\...\Run via auto-launch
                info!(autostart = a, "Autostart toggled (no-op until step 14)");
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Close-to-tray: if the user clicked the window's X, swallow the
        // close and hide instead. Quit menu sets `quitting` first so a
        // real exit goes through.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quitting {
                // Let it close; eframe::run_native will return.
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.main_visible = false;
            }
        }

        // Drain reminder fires from the tokio side and open popups.
        while let Ok(fire) = self.ui_rx.try_recv() {
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

        // Drain tray menu events.
        let menu_rx = MenuEvent::receiver();
        while let Ok(menu_event) = menu_rx.try_recv() {
            if menu_event.id() == &self.tray.quit_id {
                info!("Quit menu clicked; closing root viewport");
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // Drain tray icon click events: left-click toggles main window.
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

        // Render the main-window contents only when visible (skipping
        // the tree when hidden saves a bit of egui work per frame).
        if self.main_visible {
            let actions = main_window::render(ui, &mut self.window_state, &self.reminders);
            for action in actions {
                self.handle_main_window_action(action);
            }
        }

        // Render or evict popups.
        self.popups.retain_mut(|popup| popup.show(&ctx));

        // egui only repaints on input by default; we drive it ourselves so
        // try_recv keeps draining and 30s auto-dismiss still fires.
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
