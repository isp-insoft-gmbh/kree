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
use crate::config::PopupPosition;
use crate::theme::ThemeMode;

pub const POPUP_WIDTH: f32 = 320.0;
pub const POPUP_HEIGHT: f32 = 132.0;

const AUTO_DISMISS: Duration = Duration::from_secs(30);
const FADE_IN: Duration = Duration::from_millis(180);
const TRAY_GAP: f32 = 8.0;
const STACK_GAP: f32 = 8.0;
const FRAME_INSET: f32 = 1.0;

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
    /// Theme mode at the moment the popup opened. We freeze it for the
    /// popup's lifetime — switching the global theme mid-popup would
    /// otherwise re-tint the visible rectangle.
    theme: ThemeMode,
}

impl PopupHandle {
    pub fn new(fire: UiFire, position: (f32, f32), theme: ThemeMode) -> Self {
        let now = Instant::now();
        Self {
            inner: Arc::new(PopupData {
                icon: fire.icon,
                body: fire.body,
                fired_at: fire.fired_at,
                visible: fire.visible,
                opened_at: now,
                theme,
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
            // Each viewport carries its own Context, so the theme applied
            // to the root viewport doesn't propagate. Re-apply per frame
            // using the popup's frozen mode.
            crate::theme::apply(ctx, popup.theme);
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

    egui::Area::new(egui::Id::new("popup-content"))
        .fixed_pos(frame_pos())
        .show(ctx, |ui| {
            ui.set_min_size(frame_min_size());
            ui.multiply_opacity(alpha);
            let visuals = &ctx.global_style().visuals;
            egui::Frame::default()
                .fill(visuals.window_fill())
                .stroke(visuals.window_stroke())
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_min_size(frame_body_min_size());
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
        });

    // Drive the fade until it completes; egui won't repaint on its own.
    if alpha < 1.0 {
        ctx.request_repaint();
    }
}

fn frame_pos() -> egui::Pos2 {
    egui::pos2(FRAME_INSET, FRAME_INSET)
}

fn frame_min_size() -> egui::Vec2 {
    egui::vec2(
        POPUP_WIDTH - FRAME_INSET * 2.0,
        POPUP_HEIGHT - FRAME_INSET * 2.0,
    )
}

fn frame_body_min_size() -> egui::Vec2 {
    egui::vec2(
        POPUP_WIDTH - FRAME_INSET * 2.0 - 24.0,
        POPUP_HEIGHT - FRAME_INSET * 2.0 - 20.0,
    )
}

/// Compute the top-left position for a new popup given the currently
/// active popups, the tray-rect anchor (if known), and the user's
/// configured `PopupPosition`. The first popup sits at the chosen
/// anchor; each subsequent popup stacks toward the screen center
/// (top-anchored stacks downward, bottom / center / side-center
/// stacks upward). 8 px gap between stacked popups.
pub fn compute_position(
    existing: &[PopupHandle],
    tray_anchor: Option<(f32, f32)>,
    position: PopupPosition,
) -> (f32, f32) {
    let work_area = work_area();
    let (base, stack_up) = match position {
        PopupPosition::Tray => {
            // Existing behavior: anchored above the tray, screen-corner fallback.
            let anchor_pos = match tray_anchor {
                Some((cx, top)) => (cx - POPUP_WIDTH / 2.0, top - POPUP_HEIGHT - TRAY_GAP),
                None => fallback_position_from(&work_area),
            };
            (anchor_pos, true)
        }
        other => (
            anchor_from_work_area(&work_area, other),
            stack_up_for(other),
        ),
    };

    if existing.is_empty() {
        return base;
    }

    let next_y = if stack_up {
        let topmost = existing
            .iter()
            .map(PopupHandle::top_y)
            .fold(f32::INFINITY, f32::min);
        topmost - POPUP_HEIGHT - STACK_GAP
    } else {
        let bottommost = existing
            .iter()
            .map(PopupHandle::top_y)
            .fold(f32::NEG_INFINITY, f32::max);
        bottommost + POPUP_HEIGHT + STACK_GAP
    };
    (base.0, next_y)
}

fn stack_up_for(position: PopupPosition) -> bool {
    matches!(
        position,
        PopupPosition::BottomLeft
            | PopupPosition::BottomCenter
            | PopupPosition::BottomRight
            | PopupPosition::Center
            | PopupPosition::LeftCenter
            | PopupPosition::RightCenter
    )
}

fn anchor_from_work_area(work: &WorkArea, position: PopupPosition) -> (f32, f32) {
    let cx_screen = (work.left + work.right) / 2.0 - POPUP_WIDTH / 2.0;
    let cy_screen = (work.top + work.bottom) / 2.0 - POPUP_HEIGHT / 2.0;
    let left = work.left + EDGE_GAP;
    let right = work.right - POPUP_WIDTH - EDGE_GAP;
    let top = work.top + EDGE_GAP;
    let bottom = work.bottom - POPUP_HEIGHT - EDGE_GAP;
    match position {
        PopupPosition::Tray => (right, bottom), // shouldn't happen — caller guards
        PopupPosition::TopLeft => (left, top),
        PopupPosition::TopCenter => (cx_screen, top),
        PopupPosition::TopRight => (right, top),
        PopupPosition::LeftCenter => (left, cy_screen),
        PopupPosition::Center => (cx_screen, cy_screen),
        PopupPosition::RightCenter => (right, cy_screen),
        PopupPosition::BottomLeft => (left, bottom),
        PopupPosition::BottomCenter => (cx_screen, bottom),
        PopupPosition::BottomRight => (right, bottom),
    }
}

const EDGE_GAP: f32 = 16.0;

#[derive(Debug)]
struct WorkArea {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

fn work_area() -> WorkArea {
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
        WorkArea {
            left: rect.left as f32,
            top: rect.top as f32,
            right: rect.right as f32,
            bottom: rect.bottom as f32,
        }
    } else {
        // Last-ditch: 1920x1080 minus a typical taskbar.
        WorkArea {
            left: 0.0,
            top: 0.0,
            right: 1920.0,
            bottom: 1040.0,
        }
    }
}

fn fallback_position_from(work: &WorkArea) -> (f32, f32) {
    (
        work.right - POPUP_WIDTH - TRAY_GAP * 2.0,
        work.bottom - POPUP_HEIGHT - TRAY_GAP * 2.0,
    )
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
        PopupHandle::new(fire, (100.0, top_y), ThemeMode::Dark)
    }

    #[test]
    fn popup_frame_is_inset_so_stroke_is_not_clipped() {
        assert_eq!(frame_pos(), egui::pos2(1.0, 1.0));
        assert_eq!(frame_min_size(), egui::vec2(318.0, 130.0));
        assert_eq!(frame_body_min_size(), egui::vec2(294.0, 110.0));
    }

    #[test]
    fn tray_first_popup_sits_above_anchor() {
        let popups: Vec<PopupHandle> = Vec::new();
        let pos = compute_position(&popups, Some((1000.0, 1040.0)), PopupPosition::Tray);
        // x: cx - W/2 = 1000 - 160 = 840
        // y: top - H - TRAY_GAP = 1040 - 132 - 8 = 900
        assert!((pos.0 - 840.0).abs() < 0.1);
        assert!((pos.1 - 900.0).abs() < 0.1);
    }

    #[test]
    fn tray_second_popup_stacks_above_first() {
        let popups = vec![fake_handle(900.0)];
        let pos = compute_position(&popups, Some((1000.0, 1040.0)), PopupPosition::Tray);
        // y: 900 - 132 - 8 = 760
        assert!((pos.1 - 760.0).abs() < 0.1);
    }

    #[test]
    fn tray_third_popup_stacks_above_topmost_existing() {
        let popups = vec![fake_handle(900.0), fake_handle(760.0)];
        let pos = compute_position(&popups, Some((1000.0, 1040.0)), PopupPosition::Tray);
        // y: 760 - 132 - 8 = 620
        assert!((pos.1 - 620.0).abs() < 0.1);
    }

    #[test]
    fn tray_dismissed_middle_popup_leaves_gap() {
        // Middle popup at y=760 was dismissed and removed; new popup
        // should still go above the topmost remaining (y=620), not
        // refill the gap.
        let popups = vec![fake_handle(900.0), fake_handle(620.0)];
        let pos = compute_position(&popups, Some((1000.0, 1040.0)), PopupPosition::Tray);
        // y: 620 - 132 - 8 = 480
        assert!((pos.1 - 480.0).abs() < 0.1);
    }

    fn anchor_for_test(position: PopupPosition) -> (f32, f32) {
        // Pure helper that bypasses Win32 — assert against a fake
        // 1920x1040 work area (taskbar at bottom).
        let work = WorkArea {
            left: 0.0,
            top: 0.0,
            right: 1920.0,
            bottom: 1040.0,
        };
        anchor_from_work_area(&work, position)
    }

    #[test]
    fn fixed_anchors_against_fake_work_area() {
        // POPUP_WIDTH=320, POPUP_HEIGHT=132, EDGE_GAP=16
        // work: 0,0 → 1920,1040
        // top:    16
        // bottom: 1040 - 132 - 16 = 892
        // left:   16
        // right:  1920 - 320 - 16 = 1584
        // cx_screen: 960 - 160 = 800
        // cy_screen: 520 - 66  = 454
        let cases: &[(PopupPosition, (f32, f32))] = &[
            (PopupPosition::TopLeft, (16.0, 16.0)),
            (PopupPosition::TopCenter, (800.0, 16.0)),
            (PopupPosition::TopRight, (1584.0, 16.0)),
            (PopupPosition::LeftCenter, (16.0, 454.0)),
            (PopupPosition::Center, (800.0, 454.0)),
            (PopupPosition::RightCenter, (1584.0, 454.0)),
            (PopupPosition::BottomLeft, (16.0, 892.0)),
            (PopupPosition::BottomCenter, (800.0, 892.0)),
            (PopupPosition::BottomRight, (1584.0, 892.0)),
        ];
        for (pos, expected) in cases {
            let actual = anchor_for_test(*pos);
            assert!(
                (actual.0 - expected.0).abs() < 1.0 && (actual.1 - expected.1).abs() < 1.0,
                "{pos:?}: got {actual:?}, expected {expected:?}",
            );
        }
    }

    #[test]
    fn top_anchored_stacks_downward() {
        // top_left first popup at (16, 16); second should land at
        // y = 16 + 132 + 8 = 156.
        let first = fake_handle(16.0);
        let pos = compute_position(&[first], None, PopupPosition::TopLeft);
        assert!((pos.1 - 156.0).abs() < 1.0, "{pos:?}");
    }

    #[test]
    fn bottom_anchored_stacks_upward() {
        // bottom_right first popup with top = 892; second at
        // y = 892 - 132 - 8 = 752.
        let first = fake_handle(892.0);
        let pos = compute_position(&[first], None, PopupPosition::BottomRight);
        assert!((pos.1 - 752.0).abs() < 1.0, "{pos:?}");
    }
}
