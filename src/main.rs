mod audio;
mod logging;
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

    // Hidden, off-screen, undecorated, taskbar-less main viewport. The
    // root viewport is required by eframe but never shown — popups open
    // as deferred child viewports. Step 11 will give it a real role
    // (the kree main window).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)
            .with_taskbar(false)
            .with_decorations(false)
            .with_inner_size([1.0, 1.0])
            .with_position([-32000.0, -32000.0]),
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
}

impl App {
    fn new(ui_rx: mpsc::UnboundedReceiver<UiFire>) -> Result<Self> {
        // Tray must be built on the same thread that pumps Win32 messages
        // (the eframe / winit thread). The creator closure runs there.
        let tray = tray::build()?;
        Ok(Self {
            tray,
            ui_rx,
            popups: Vec::new(),
        })
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Drain reminder fires from the tokio side and open popups.
        while let Ok(fire) = self.ui_rx.try_recv() {
            let anchor = self.tray.rect_anchor();
            info!(
                icon = %fire.icon,
                body = %fire.body,
                anchor = ?anchor,
                "opening popup"
            );
            self.popups.push(popup::PopupHandle::new(fire, anchor));
        }

        // Drain tray menu events.
        let menu_rx = MenuEvent::receiver();
        while let Ok(menu_event) = menu_rx.try_recv() {
            if menu_event.id() == &self.tray.quit_id {
                info!("Quit menu clicked; closing root viewport");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // Render or evict popups.
        self.popups.retain_mut(|popup| popup.show(&ctx));

        // egui only repaints on input by default; we drive it ourselves so
        // try_recv keeps draining and 30s auto-dismiss still fires.
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
