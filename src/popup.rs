use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use eframe::egui::{self, ViewportBuilder, ViewportCommand, ViewportId};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

use crate::UiFire;
use crate::config::PopupPosition;
use crate::theme::ThemeMode;

/// A screen rectangle in physical pixels.
///
/// Used for the tray icon's bounds. `tray-icon` reports position and size;
/// we keep all four edges because which *side* of the icon the popup goes on
/// depends on where the icon sits relative to the desktop work area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl ScreenRect {
    fn center_x(&self) -> f32 {
        (self.left + self.right) / 2.0
    }

    fn center_y(&self) -> f32 {
        (self.top + self.bottom) / 2.0
    }
}

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

        let builder = popup_viewport(self.position);

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

/// The viewport a popup is shown in.
///
/// `with_active(false)` is the load-bearing one, and the easiest to lose in a
/// refactor because nothing about it is visible on a developer's machine
/// unless they are typing in another window when a reminder fires.
///
/// Without it, `egui-winit` defaults to `active: true`, winit sets its
/// `MARKER_ACTIVATE` flag, and the window is shown with Win32 `SW_SHOW` —
/// which takes the foreground and pulls focus out of whatever the user was
/// working in. With it, winit shows the window with `SW_SHOWNOACTIVATE`
/// instead: the popup appears on top, does not steal focus, and still
/// activates normally when clicked, so Dismiss keeps working.
///
/// `docs/specs/spec.md` § 5.4 requires this ("no focus steal").
fn popup_viewport(position: (f32, f32)) -> ViewportBuilder {
    ViewportBuilder::default()
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top()
        .with_taskbar(false)
        .with_active(false)
        .with_position(position)
        .with_inner_size([POPUP_WIDTH, POPUP_HEIGHT])
}

fn render(ctx: &egui::Context, popup: &Arc<PopupData>) {
    let elapsed = popup.opened_at.elapsed();
    let alpha = if elapsed >= FADE_IN {
        1.0
    } else {
        elapsed.as_secs_f32() / FADE_IN.as_secs_f32()
    };

    // `Id::unique` is the renamed `Id::new` — egui 0.37 deprecated the latter,
    // and it already delegated here, so this is the same id. "Unique" means a
    // root id rather than a salt derived from a parent, which is what
    // `Area::new` wants; each popup is its own viewport with its own Context,
    // so the constant source does not collide across popups.
    egui::Area::new(egui::Id::unique("popup-content"))
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
    tray_icon: Option<ScreenRect>,
    position: PopupPosition,
) -> (f32, f32) {
    let (base, stack_up) = match position {
        PopupPosition::Tray => match tray_icon {
            // The work area must come from the monitor the tray icon is
            // actually on. `SPI_GETWORKAREA` only ever reports the primary
            // monitor's, while the icon rect is in virtual-screen
            // coordinates — mixing the two puts the popup on the wrong
            // screen entirely when the taskbar is on a secondary monitor.
            Some(icon) => {
                let work = work_area_for_icon(&icon);
                tray_placement(&icon, &work)
            }
            None => (fallback_position_from(&primary_work_area()), true),
        },
        // The fixed positions name a place on the user's screen, so they
        // stay on the primary monitor as before.
        other => (
            anchor_from_work_area(&primary_work_area(), other),
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

/// Which screen edge the taskbar is docked to, inferred from geometry.
///
/// The work area is the desktop minus the taskbar, and the tray icon lives
/// *inside* the taskbar — so the icon falls outside the work area on exactly
/// the edge the taskbar occupies. That identifies all four positions without
/// asking Win32 which one it is, and degrades to `Unknown` rather than
/// guessing when the two overlap (an auto-hiding taskbar leaves the work area
/// covering the whole screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskbarEdge {
    Bottom,
    Top,
    Left,
    Right,
    Unknown,
}

fn taskbar_edge(icon: &ScreenRect, work: &WorkArea) -> TaskbarEdge {
    if icon.top >= work.bottom {
        TaskbarEdge::Bottom
    } else if icon.bottom <= work.top {
        TaskbarEdge::Top
    } else if icon.right <= work.left {
        TaskbarEdge::Left
    } else if icon.left >= work.right {
        TaskbarEdge::Right
    } else {
        TaskbarEdge::Unknown
    }
}

/// Where a tray-anchored popup goes, and which way further popups stack.
///
/// The popup sits beside the tray icon on whichever side faces the work
/// area, aligned to the icon along the taskbar's own axis. Placing it
/// unconditionally *above* the icon — which is what this did before — is
/// only correct for a bottom taskbar: with the taskbar on the left, the
/// icon's centre x is a few dozen pixels, so centring a 320 px popup on it
/// put more than half of it off the left edge of the screen.
///
/// Every result is clamped into the work area, so a popup cannot leave the
/// screen even if the icon rect is reported somewhere unexpected.
fn tray_placement(icon: &ScreenRect, work: &WorkArea) -> ((f32, f32), bool) {
    let (raw, stack_up) = match taskbar_edge(icon, work) {
        // Above the icon, stacking up into the desktop.
        TaskbarEdge::Bottom => (
            (
                icon.center_x() - POPUP_WIDTH / 2.0,
                work.bottom - POPUP_HEIGHT - TRAY_GAP,
            ),
            true,
        ),
        // Below the icon. The room is downward, so stack that way.
        TaskbarEdge::Top => (
            (icon.center_x() - POPUP_WIDTH / 2.0, work.top + TRAY_GAP),
            false,
        ),
        // Beside the icon, vertically centred on it.
        TaskbarEdge::Left => (
            (work.left + TRAY_GAP, icon.center_y() - POPUP_HEIGHT / 2.0),
            true,
        ),
        TaskbarEdge::Right => (
            (
                work.right - POPUP_WIDTH - TRAY_GAP,
                icon.center_y() - POPUP_HEIGHT / 2.0,
            ),
            true,
        ),
        TaskbarEdge::Unknown => (fallback_position_from(work), true),
    };
    (clamp_into(work, raw, TRAY_GAP), stack_up)
}

/// Keep a position inside the work area, leaving `margin` at each edge.
///
/// The `max` guards the degenerate case where the work area is narrower or
/// shorter than the popup: without it `clamp` would be called with min > max
/// and panic.
fn clamp_into(work: &WorkArea, (x, y): (f32, f32), margin: f32) -> (f32, f32) {
    let min_x = work.left + margin;
    let min_y = work.top + margin;
    let max_x = (work.right - POPUP_WIDTH - margin).max(min_x);
    let max_y = (work.bottom - POPUP_HEIGHT - margin).max(min_y);
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
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

/// Work area of the monitor the tray icon sits on.
///
/// Falls back to the primary monitor's when the lookup fails, which is no
/// worse than what this did before the monitor was consulted at all.
fn work_area_for_icon(icon: &ScreenRect) -> WorkArea {
    let rect = RECT {
        left: icon.left as i32,
        top: icon.top as i32,
        right: icon.right as i32,
        bottom: icon.bottom as i32,
    };
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_invalid() {
        return primary_work_area();
    }
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let ok = unsafe { GetMonitorInfoW(monitor, &mut info) };
    if !ok.as_bool() {
        return primary_work_area();
    }
    WorkArea {
        left: info.rcWork.left as f32,
        top: info.rcWork.top as f32,
        right: info.rcWork.right as f32,
        bottom: info.rcWork.bottom as f32,
    }
}

fn primary_work_area() -> WorkArea {
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

    /// A reminder must never take focus off what the user is doing.
    ///
    /// This cannot assert the Win32 behaviour from here — it pins the flag
    /// that produces it. The bug this guards against was simply the absence
    /// of `with_active(false)`, which is invisible in review and invisible
    /// at runtime unless another window happens to be focused when a
    /// reminder fires. See `popup_viewport` and `docs/specs/spec.md` § 5.4.
    #[test]
    fn popup_viewport_does_not_take_focus() {
        let builder = popup_viewport((100.0, 200.0));
        assert_eq!(
            builder.active,
            Some(false),
            "popup must be shown with SW_SHOWNOACTIVATE, not SW_SHOW"
        );
    }

    /// The rest of § 5.4's window contract, pinned alongside it: frameless,
    /// always on top, and absent from the taskbar.
    #[test]
    fn popup_viewport_is_frameless_topmost_and_off_the_taskbar() {
        let builder = popup_viewport((100.0, 200.0));
        assert_eq!(builder.decorations, Some(false));
        assert_eq!(builder.taskbar, Some(false));
        assert_eq!(builder.window_level, Some(egui::WindowLevel::AlwaysOnTop));
    }

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

    /// A 1920x1080 screen with the taskbar on the given edge, and the tray
    /// icon sitting inside that taskbar. `work` excludes the taskbar, which
    /// is what Win32 reports and what the placement logic reads.
    fn screen_with_taskbar(edge: TaskbarEdge) -> (ScreenRect, WorkArea) {
        match edge {
            TaskbarEdge::Bottom => (
                ScreenRect {
                    left: 984.0,
                    top: 1044.0,
                    right: 1016.0,
                    bottom: 1076.0,
                },
                WorkArea {
                    left: 0.0,
                    top: 0.0,
                    right: 1920.0,
                    bottom: 1040.0,
                },
            ),
            TaskbarEdge::Top => (
                ScreenRect {
                    left: 984.0,
                    top: 4.0,
                    right: 1016.0,
                    bottom: 36.0,
                },
                WorkArea {
                    left: 0.0,
                    top: 40.0,
                    right: 1920.0,
                    bottom: 1080.0,
                },
            ),
            TaskbarEdge::Left => (
                ScreenRect {
                    left: 20.0,
                    top: 500.0,
                    right: 52.0,
                    bottom: 532.0,
                },
                WorkArea {
                    left: 72.0,
                    top: 0.0,
                    right: 1920.0,
                    bottom: 1080.0,
                },
            ),
            TaskbarEdge::Right => (
                ScreenRect {
                    left: 1868.0,
                    top: 500.0,
                    right: 1900.0,
                    bottom: 532.0,
                },
                WorkArea {
                    left: 0.0,
                    top: 0.0,
                    right: 1848.0,
                    bottom: 1080.0,
                },
            ),
            TaskbarEdge::Unknown => (
                // Auto-hidden taskbar: the work area covers the whole screen,
                // so the icon rect overlaps it and no edge can be inferred.
                ScreenRect {
                    left: 984.0,
                    top: 1044.0,
                    right: 1016.0,
                    bottom: 1076.0,
                },
                WorkArea {
                    left: 0.0,
                    top: 0.0,
                    right: 1920.0,
                    bottom: 1080.0,
                },
            ),
        }
    }

    fn bottom_taskbar_icon() -> ScreenRect {
        // Matches the pre-existing expectations: centre x 1000, and a work
        // area bottom of 1040 once Win32 excludes the taskbar.
        ScreenRect {
            left: 984.0,
            top: 1044.0,
            right: 1016.0,
            bottom: 1076.0,
        }
    }

    #[test]
    fn taskbar_edge_is_inferred_for_every_docking_position() {
        for edge in [
            TaskbarEdge::Bottom,
            TaskbarEdge::Top,
            TaskbarEdge::Left,
            TaskbarEdge::Right,
        ] {
            let (icon, work) = screen_with_taskbar(edge);
            assert_eq!(taskbar_edge(&icon, &work), edge, "{edge:?}");
        }
        let (icon, work) = screen_with_taskbar(TaskbarEdge::Unknown);
        assert_eq!(taskbar_edge(&icon, &work), TaskbarEdge::Unknown);
    }

    #[test]
    fn tray_first_popup_sits_above_anchor() {
        // Bottom taskbar — unchanged behaviour.
        // x: centre 1000 - W/2 = 840
        // y: work.bottom - H - TRAY_GAP = 1040 - 132 - 8 = 900
        let (icon, work) = screen_with_taskbar(TaskbarEdge::Bottom);
        let (pos, stack_up) = tray_placement(&icon, &work);
        assert!((pos.0 - 840.0).abs() < 0.1, "{pos:?}");
        assert!((pos.1 - 900.0).abs() < 0.1, "{pos:?}");
        assert!(stack_up);
    }

    /// The reported bug: with the taskbar on the left, centring the popup on
    /// the icon's x put it mostly off the left edge of the screen.
    #[test]
    fn left_taskbar_puts_the_popup_beside_it_not_off_screen() {
        let (icon, work) = screen_with_taskbar(TaskbarEdge::Left);
        let (pos, _) = tray_placement(&icon, &work);
        // Beside the taskbar: work.left + TRAY_GAP = 72 + 8 = 80.
        assert!((pos.0 - 80.0).abs() < 0.1, "{pos:?}");
        // Vertically centred on the icon: 516 - 66 = 450.
        assert!((pos.1 - 450.0).abs() < 0.1, "{pos:?}");
        // The old code produced 20 + 16 - 160 = negative x.
        assert!(
            pos.0 > 0.0,
            "popup must not hang off the left edge: {pos:?}"
        );
    }

    #[test]
    fn right_taskbar_puts_the_popup_beside_it() {
        let (icon, work) = screen_with_taskbar(TaskbarEdge::Right);
        let (pos, _) = tray_placement(&icon, &work);
        // work.right - W - TRAY_GAP = 1848 - 320 - 8 = 1520
        assert!((pos.0 - 1520.0).abs() < 0.1, "{pos:?}");
        assert!((pos.1 - 450.0).abs() < 0.1, "{pos:?}");
    }

    #[test]
    fn top_taskbar_puts_the_popup_below_it_and_stacks_downward() {
        let (icon, work) = screen_with_taskbar(TaskbarEdge::Top);
        let (pos, stack_up) = tray_placement(&icon, &work);
        assert!((pos.0 - 840.0).abs() < 0.1, "{pos:?}");
        // work.top + TRAY_GAP = 40 + 8 = 48
        assert!((pos.1 - 48.0).abs() < 0.1, "{pos:?}");
        // Anchored at the top, so the only room is downward.
        assert!(!stack_up, "a top taskbar must stack popups downward");
    }

    /// Whatever the taskbar does, the popup stays on the desktop.
    #[test]
    fn every_taskbar_edge_keeps_the_popup_inside_the_work_area() {
        for edge in [
            TaskbarEdge::Bottom,
            TaskbarEdge::Top,
            TaskbarEdge::Left,
            TaskbarEdge::Right,
            TaskbarEdge::Unknown,
        ] {
            let (icon, work) = screen_with_taskbar(edge);
            let (pos, _) = tray_placement(&icon, &work);
            assert!(pos.0 >= work.left, "{edge:?} left: {pos:?}");
            assert!(pos.1 >= work.top, "{edge:?} top: {pos:?}");
            assert!(pos.0 + POPUP_WIDTH <= work.right, "{edge:?} right: {pos:?}");
            assert!(
                pos.1 + POPUP_HEIGHT <= work.bottom,
                "{edge:?} bottom: {pos:?}"
            );
        }
    }

    /// A work area smaller than the popup must clamp rather than panic.
    #[test]
    fn absurdly_small_work_area_does_not_panic() {
        let work = WorkArea {
            left: 0.0,
            top: 0.0,
            right: 100.0,
            bottom: 50.0,
        };
        let icon = ScreenRect {
            left: 40.0,
            top: 60.0,
            right: 60.0,
            bottom: 80.0,
        };
        let (pos, _) = tray_placement(&icon, &work);
        assert!(pos.0.is_finite() && pos.1.is_finite(), "{pos:?}");
    }

    #[test]
    fn tray_second_popup_stacks_above_first() {
        let popups = vec![fake_handle(900.0)];
        let pos = compute_position(&popups, Some(bottom_taskbar_icon()), PopupPosition::Tray);
        // y: 900 - 132 - 8 = 760
        assert!((pos.1 - 760.0).abs() < 0.1);
    }

    #[test]
    fn tray_third_popup_stacks_above_topmost_existing() {
        let popups = vec![fake_handle(900.0), fake_handle(760.0)];
        let pos = compute_position(&popups, Some(bottom_taskbar_icon()), PopupPosition::Tray);
        // y: 760 - 132 - 8 = 620
        assert!((pos.1 - 620.0).abs() < 0.1);
    }

    #[test]
    fn tray_dismissed_middle_popup_leaves_gap() {
        // Middle popup at y=760 was dismissed and removed; new popup
        // should still go above the topmost remaining (y=620), not
        // refill the gap.
        let popups = vec![fake_handle(900.0), fake_handle(620.0)];
        let pos = compute_position(&popups, Some(bottom_taskbar_icon()), PopupPosition::Tray);
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
