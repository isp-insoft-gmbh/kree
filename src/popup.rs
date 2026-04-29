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
const FADE_IN: Duration = Duration::from_millis(180);
const TRAY_GAP: f32 = 8.0;
const STACK_GAP: f32 = 8.0;

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
    opened_at: Instant,
}

impl PopupHandle {
    pub fn new(fire: UiFire, position: (f32, f32)) -> Self {
        let now = Instant::now();
        Self {
            inner: Arc::new(PopupData {
                icon: fire.icon,
                body: fire.body,
                fired_at: fire.fired_at,
                visible: fire.visible,
                opened_at: now,
            }),
            opened_at: now,
            position,
        }
    }

    fn top_y(&self) -> f32 {
        self.position.1
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
    let elapsed = popup.opened_at.elapsed();
    let alpha = if elapsed >= FADE_IN {
        1.0
    } else {
        elapsed.as_secs_f32() / FADE_IN.as_secs_f32()
    };

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
            ui.multiply_opacity(alpha);
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

    // Drive the fade until it completes; egui won't repaint on its own.
    if alpha < 1.0 {
        ctx.request_repaint();
    }
}

/// Compute the top-left position for a new popup given the currently
/// active popups. The first popup sits just above the tray (or in the
/// fallback corner); each subsequent popup stacks above the topmost
/// existing one with an 8 px gap (SPEC.md § 5.4).
///
/// We don't reflow when popups in the middle are dismissed — leaves a
/// gap, which the spec doesn't forbid and avoids visual jitter.
pub fn compute_position(existing: &[PopupHandle], anchor: Option<(f32, f32)>) -> (f32, f32) {
    let base = match anchor {
        Some((cx, top)) => (cx - POPUP_WIDTH / 2.0, top - POPUP_HEIGHT - TRAY_GAP),
        None => fallback_position(),
    };
    let highest = existing
        .iter()
        .map(PopupHandle::top_y)
        .fold(f32::INFINITY, f32::min);
    if highest.is_finite() {
        (base.0, highest - POPUP_HEIGHT - STACK_GAP)
    } else {
        base
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_handle(top_y: f32) -> PopupHandle {
        let visible = Arc::new(AtomicBool::new(true));
        let fire = UiFire {
            icon: "🔬".into(),
            body: "test".into(),
            fired_at: Local::now(),
            visible,
        };
        PopupHandle::new(fire, (100.0, top_y))
    }

    #[test]
    fn first_popup_sits_above_tray_anchor() {
        let popups: Vec<PopupHandle> = Vec::new();
        let pos = compute_position(&popups, Some((1000.0, 1040.0)));
        // x: cx - W/2 = 1000 - 160 = 840
        // y: top - H - TRAY_GAP = 1040 - 100 - 8 = 932
        assert!((pos.0 - 840.0).abs() < 0.1);
        assert!((pos.1 - 932.0).abs() < 0.1);
    }

    #[test]
    fn second_popup_stacks_above_first() {
        let popups = vec![fake_handle(932.0)];
        let pos = compute_position(&popups, Some((1000.0, 1040.0)));
        // y: 932 - 100 - 8 = 824
        assert!((pos.1 - 824.0).abs() < 0.1);
    }

    #[test]
    fn third_popup_stacks_above_topmost_existing() {
        let popups = vec![fake_handle(932.0), fake_handle(824.0)];
        let pos = compute_position(&popups, Some((1000.0, 1040.0)));
        // y: 824 - 100 - 8 = 716
        assert!((pos.1 - 716.0).abs() < 0.1);
    }

    #[test]
    fn dismissed_middle_popup_leaves_gap() {
        // Middle popup at y=824 was dismissed and removed; new popup
        // should still go above the topmost remaining (y=716), not
        // refill the gap.
        let popups = vec![fake_handle(932.0), fake_handle(716.0)];
        let pos = compute_position(&popups, Some((1000.0, 1040.0)));
        // y: 716 - 100 - 8 = 608
        assert!((pos.1 - 608.0).abs() < 0.1);
    }
}
