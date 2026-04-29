//! kree's egui palette, mapped from the user's nugu theme schema
//! (`~/Workspaces/dotfiles/nugu/base.toml`). Each palette slot has a
//! semantic role in nugu — we map it to the closest egui slot:
//!
//! | nugu       | egui                                |
//! |------------|-------------------------------------|
//! | normal     | `override_text_color`               |
//! | backdrop   | window / panel / extreme background |
//! | accent     | active widget fill, prominent stroke |
//! | important  | inactive widget border, hover fill  |
//! | focus      | selection + focused stroke          |
//! | error      | `error_fg_color`                    |
//! | info       | `hyperlink_color`                   |
//! | warning    | `warn_fg_color`                     |
//!
//! `apply` is idempotent — the popup's deferred viewport calls it on every
//! frame so each viewport (which has its own egui `Context`) inherits the
//! same look. The cost is a `Visuals` clone per repaint; small and fine.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

const NORMAL: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
const BACKDROP: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
const ACCENT: Color32 = Color32::from_rgb(0xff, 0x00, 0xff);
const IMPORTANT: Color32 = Color32::from_rgb(0x00, 0x00, 0xff);
const FOCUS: Color32 = Color32::from_rgb(0x00, 0xff, 0x00);
const ERROR: Color32 = Color32::from_rgb(0xff, 0x00, 0x00);
const INFO: Color32 = Color32::from_rgb(0x00, 0xff, 0xff);
const WARN: Color32 = Color32::from_rgb(0xff, 0xff, 0x00);

const FAINT: Color32 = Color32::from_rgb(0x10, 0x10, 0x14);
const SUBTLE: Color32 = Color32::from_rgb(0x18, 0x18, 0x1c);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();

    v.override_text_color = Some(NORMAL);
    v.window_fill = BACKDROP;
    v.panel_fill = BACKDROP;
    v.extreme_bg_color = BACKDROP;
    v.faint_bg_color = FAINT;
    v.code_bg_color = FAINT;
    v.error_fg_color = ERROR;
    v.warn_fg_color = WARN;
    v.hyperlink_color = INFO;
    v.window_stroke = Stroke::new(1.0, ACCENT);
    v.selection.bg_fill = FOCUS;
    v.selection.stroke = Stroke::new(1.0, FOCUS);

    let radius = CornerRadius::same(2);
    let widgets = &mut v.widgets;

    widgets.noninteractive.bg_fill = BACKDROP;
    widgets.noninteractive.weak_bg_fill = BACKDROP;
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, NORMAL);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, SUBTLE);
    widgets.noninteractive.corner_radius = radius;

    widgets.inactive.bg_fill = SUBTLE;
    widgets.inactive.weak_bg_fill = FAINT;
    widgets.inactive.fg_stroke = Stroke::new(1.0, NORMAL);
    widgets.inactive.bg_stroke = Stroke::new(1.0, IMPORTANT);
    widgets.inactive.corner_radius = radius;

    widgets.hovered.bg_fill = IMPORTANT;
    widgets.hovered.weak_bg_fill = SUBTLE;
    widgets.hovered.fg_stroke = Stroke::new(1.5, NORMAL);
    widgets.hovered.bg_stroke = Stroke::new(1.5, FOCUS);
    widgets.hovered.corner_radius = radius;

    widgets.active.bg_fill = ACCENT;
    widgets.active.weak_bg_fill = SUBTLE;
    widgets.active.fg_stroke = Stroke::new(1.5, BACKDROP);
    widgets.active.bg_stroke = Stroke::new(1.5, FOCUS);
    widgets.active.corner_radius = radius;

    widgets.open.bg_fill = IMPORTANT;
    widgets.open.fg_stroke = Stroke::new(1.0, NORMAL);
    widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);
    widgets.open.corner_radius = radius;

    ctx.set_visuals(v);
}
