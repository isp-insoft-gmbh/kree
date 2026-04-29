use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use eframe::egui::{self, ViewportBuilder, ViewportCommand, ViewportId};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

use crate::UiFire;

pub const POPUP_WIDTH: f32 = 320.0;
pub const POPUP_HEIGHT: f32 = 100.0;

const AUTO_DISMISS: Duration = Duration::from_secs(30);
const TRAY_GAP: f32 = 8.0;

/// One live popup. The shared `visible` flag is observed by the 2-second
/// TTS gate (in `run_async`) to decide whether to speak.
pub struct PopupHandle {
    inner: Arc<PopupData>,
    opened_at: Instant,
    /// Top-left position in physical pixels, baked at creation time so the
    /// popup doesn't jump around if the tray rect changes mid-life.
    position: (f32, f32),
}

struct PopupData {
    icon: String,
    body: String,
    fired_at: DateTime<Local>,
    visible: Arc<AtomicBool>,
}

impl PopupHandle {
    pub fn new(fire: UiFire, anchor: Option<(f32, f32)>) -> Self {
        let position = match anchor {
            Some((cx, top)) => (cx - POPUP_WIDTH / 2.0, top - POPUP_HEIGHT - TRAY_GAP),
            None => fallback_position(),
        };
        Self {
            inner: Arc::new(PopupData {
                icon: fire.icon,
                body: fire.body,
                fired_at: fire.fired_at,
                visible: fire.visible,
            }),
            opened_at: Instant::now(),
            position,
        }
    }

    /// Render this popup as a deferred viewport. Returns `true` if the
    /// popup should remain in the parent's list, `false` to evict.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if self.opened_at.elapsed() >= AUTO_DISMISS {
            self.inner.visible.store(false, Ordering::Release);
        }
        if !self.inner.visible.load(Ordering::Acquire) {
            return false;
        }

        let viewport_id = ViewportId::from_hash_of(format!(
            "kree-popup-{}",
            self.inner.fired_at.timestamp_nanos_opt().unwrap_or(0)
        ));

        let builder = ViewportBuilder::default()
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_position(self.position)
            .with_inner_size([POPUP_WIDTH, POPUP_HEIGHT]);

        let popup = Arc::clone(&self.inner);
        ctx.show_viewport_deferred(viewport_id, builder, move |ctx, _class| {
            render(ctx, &popup);
        });

        true
    }
}

fn render(ctx: &egui::Context, popup: &Arc<PopupData>) {
    // CentralPanel::show against a Context is still the canonical
    // multi-viewport entry point; the rename to show_inside is for
    // nested-Ui usage. Allow the deprecation here.
    #[allow(deprecated)]
    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(ctx.style().visuals.window_fill())
                .stroke(ctx.style().visuals.window_stroke())
                .inner_margin(egui::Margin::symmetric(12, 10)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&popup.icon).size(40.0));
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&popup.body).size(15.0));
                    ui.label(
                        egui::RichText::new(popup.fired_at.format("%H:%M:%S").to_string())
                            .size(11.0)
                            .weak(),
                    );
                    ui.add_space(4.0);
                    if ui.button("Dismiss").clicked() {
                        popup.visible.store(false, Ordering::Release);
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            });
        });
}

/// Bottom-right corner of the primary monitor's work area, in physical
/// pixels. Used when `tray.rect_anchor()` returns `None` — e.g. on
/// virtualized hosts that don't expose the tray icon's rect.
fn fallback_position() -> (f32, f32) {
    let mut rect = RECT::default();
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if ok.is_ok() {
        let x = rect.right as f32 - POPUP_WIDTH - TRAY_GAP * 2.0;
        let y = rect.bottom as f32 - POPUP_HEIGHT - TRAY_GAP * 2.0;
        return (x, y);
    }
    // Last-ditch hardcode for an unhealthy SPI call.
    (1500.0, 900.0)
}
