//! kree's egui palettes, mapped from the user's nugu themes
//! (`~/Workspaces/dotfiles/nugu/output/{dark,light}.toml`, generated
//! by `just build` from `nugu.nu` + `palette.nu`).
//!
//! `apply` is idempotent. Popup viewports call it from their own egui
//! contexts; the main window applies it only when the effective theme
//! changes.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Visuals,
};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

// -----------------------------------------------------------------
// Dark palette — output/dark.toml
// -----------------------------------------------------------------

mod dark {
    use eframe::egui::Color32;

    // palette.* (top-level)
    pub const ERROR: Color32 = Color32::from_rgb(0xe4, 0x64, 0x79);
    pub const WARN: Color32 = Color32::from_rgb(0x9e, 0x9a, 0x69);
    pub const INFO: Color32 = Color32::from_rgb(0x3e, 0xc5, 0xdd);

    // palette.ui (chrome)
    pub const NORMAL: Color32 = Color32::from_rgb(0xdb, 0xdb, 0xdb);
    pub const BACKDROP: Color32 = Color32::from_rgb(0x24, 0x24, 0x24);
    pub const ACCENT: Color32 = Color32::from_rgb(0xd8, 0x6d, 0xd8);
    pub const MINOR: Color32 = Color32::from_rgb(0x9d, 0x9d, 0x9d);
    pub const FOCUS: Color32 = Color32::from_rgb(0x9a, 0xab, 0xae);
    pub const UNFOCUS: Color32 = Color32::from_rgb(0x6f, 0x6f, 0x6f);
    pub const IMPORTANT_LOCAL: Color32 = Color32::from_rgb(0xc9, 0xb6, 0xeb);
    pub const IMPORTANT_GLOBAL: Color32 = Color32::from_rgb(0xf3, 0xef, 0xfb);

    pub const FAINT: Color32 = Color32::from_rgb(0x10, 0x10, 0x14);
    pub const SUBTLE: Color32 = Color32::from_rgb(0x18, 0x18, 0x1c);
}

// -----------------------------------------------------------------
// Light palette — output/light.toml
// -----------------------------------------------------------------

mod light {
    use eframe::egui::Color32;

    pub const ERROR: Color32 = Color32::from_rgb(0xc0, 0x22, 0x3c);
    pub const WARN: Color32 = Color32::from_rgb(0x24, 0x23, 0x17);
    pub const INFO: Color32 = Color32::from_rgb(0x13, 0x61, 0x6e);

    pub const NORMAL: Color32 = Color32::from_rgb(0x24, 0x24, 0x24);
    pub const BACKDROP: Color32 = Color32::from_rgb(0xdb, 0xdb, 0xdb);
    pub const ACCENT: Color32 = Color32::from_rgb(0x92, 0x27, 0x92);
    pub const MINOR: Color32 = Color32::from_rgb(0x29, 0x29, 0x29);
    pub const UNFOCUS: Color32 = Color32::from_rgb(0x5d, 0x5d, 0x5d);
    pub const IMPORTANT_LOCAL: Color32 = Color32::from_rgb(0x60, 0x49, 0x80);
    pub const IMPORTANT_GLOBAL: Color32 = Color32::from_rgb(0x3e, 0x2f, 0x53);

    pub const FAINT: Color32 = Color32::from_rgb(0xea, 0xea, 0xea);
    pub const SUBTLE: Color32 = Color32::from_rgb(0xcd, 0xcd, 0xcd);
}

const JETBRAINS_MONO_NF: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf");
const NOTO_EMOJI: &[u8] = include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf");

/// Register the bundled Nerd Font plus an emoji-symbol fallback.
pub fn install_fonts(ctx: &egui::Context) {
    let fonts = font_definitions();
    info!(fonts = ?["jetbrains-mono-nf", "noto-emoji"], "fonts loaded");
    ctx.set_fonts(fonts);
}

fn font_definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "jetbrains-mono-nf".into(),
        Arc::new(FontData::from_static(JETBRAINS_MONO_NF)),
    );
    fonts.font_data.insert(
        "noto-emoji".into(),
        Arc::new(FontData::from_static(NOTO_EMOJI)),
    );

    prepend_font(&mut fonts, FontFamily::Proportional, "noto-emoji");
    prepend_font(&mut fonts, FontFamily::Proportional, "jetbrains-mono-nf");
    prepend_font(&mut fonts, FontFamily::Monospace, "noto-emoji");
    prepend_font(&mut fonts, FontFamily::Monospace, "jetbrains-mono-nf");
    fonts
}

fn prepend_font(fonts: &mut FontDefinitions, family: FontFamily, name: &'static str) {
    if let Some(family) = fonts.families.get_mut(&family) {
        family.retain(|existing| existing != name);
        family.insert(0, name.into());
    }
}

fn apply_text_styles(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(26.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(14.0, FontFamily::Monospace),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
    });
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    apply_text_styles(ctx);
    let visuals = match mode {
        ThemeMode::Dark => dark_visuals(),
        ThemeMode::Light => light_visuals(),
    };
    ctx.set_visuals(visuals);
}

/// Heading / strong-text accent that the rest of the codebase uses for
/// the main-window title (and similar high-emphasis labels). Picks
/// `important_global` from whichever palette is active so callers
/// don't have to know which mode is current.
pub fn heading_color(mode: ThemeMode) -> Color32 {
    match mode {
        ThemeMode::Dark => dark::IMPORTANT_GLOBAL,
        ThemeMode::Light => light::IMPORTANT_GLOBAL,
    }
}

/// Subtitle / mid-emphasis label color.
pub fn subtitle_color(mode: ThemeMode) -> Color32 {
    match mode {
        ThemeMode::Dark => dark::IMPORTANT_LOCAL,
        ThemeMode::Light => light::IMPORTANT_LOCAL,
    }
}

/// Muted footnote color.
pub fn muted_color(mode: ThemeMode) -> Color32 {
    match mode {
        ThemeMode::Dark => dark::MINOR,
        ThemeMode::Light => light::MINOR,
    }
}

fn dark_visuals() -> Visuals {
    let mut v = Visuals::dark();

    v.window_fill = dark::BACKDROP;
    v.panel_fill = dark::BACKDROP;
    v.extreme_bg_color = Color32::from_rgb(0x12, 0x12, 0x12);
    v.faint_bg_color = Color32::from_rgb(0x2c, 0x2c, 0x2c);
    v.code_bg_color = Color32::from_rgb(0x12, 0x12, 0x12);
    v.window_stroke = Stroke::new(1.0, dark::UNFOCUS);

    v.error_fg_color = dark::ERROR;
    v.warn_fg_color = dark::WARN;
    v.hyperlink_color = dark::INFO;

    v.selection.bg_fill = dark::FOCUS;
    v.selection.stroke = Stroke::new(1.0, dark::IMPORTANT_LOCAL);

    let radius = CornerRadius::same(3);
    let widgets = &mut v.widgets;

    widgets.noninteractive.bg_fill = dark::BACKDROP;
    widgets.noninteractive.weak_bg_fill = dark::BACKDROP;
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, dark::NORMAL);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, dark::UNFOCUS);
    widgets.noninteractive.corner_radius = radius;

    widgets.inactive.bg_fill = dark::SUBTLE;
    widgets.inactive.weak_bg_fill = dark::FAINT;
    widgets.inactive.fg_stroke = Stroke::new(1.0, dark::NORMAL);
    widgets.inactive.bg_stroke = Stroke::new(1.0, dark::MINOR);
    widgets.inactive.corner_radius = radius;

    widgets.hovered.bg_fill = dark::SUBTLE;
    widgets.hovered.weak_bg_fill = dark::FAINT;
    widgets.hovered.fg_stroke = Stroke::new(1.0, dark::IMPORTANT_GLOBAL);
    widgets.hovered.bg_stroke = Stroke::new(1.0, dark::ACCENT);
    widgets.hovered.corner_radius = radius;

    widgets.active.bg_fill = dark::SUBTLE;
    widgets.active.weak_bg_fill = dark::FAINT;
    widgets.active.fg_stroke = Stroke::new(1.0, dark::IMPORTANT_GLOBAL);
    widgets.active.bg_stroke = Stroke::new(1.0, dark::ACCENT);
    widgets.active.corner_radius = radius;

    widgets.open.bg_fill = dark::SUBTLE;
    widgets.open.fg_stroke = Stroke::new(1.0, dark::IMPORTANT_GLOBAL);
    widgets.open.bg_stroke = Stroke::new(1.0, dark::ACCENT);
    widgets.open.corner_radius = radius;
    v
}

fn light_visuals() -> Visuals {
    let mut v = Visuals::light();

    v.window_fill = light::BACKDROP;
    v.panel_fill = light::BACKDROP;
    v.extreme_bg_color = light::SUBTLE;
    v.faint_bg_color = light::FAINT;
    v.code_bg_color = light::FAINT;
    v.window_stroke = Stroke::new(1.0, light::UNFOCUS);

    v.error_fg_color = light::ERROR;
    v.warn_fg_color = light::WARN;
    v.hyperlink_color = light::INFO;

    v.selection.bg_fill = Color32::from_rgb(0xc9, 0xd6, 0xda);
    v.selection.stroke = Stroke::new(1.0, light::IMPORTANT_LOCAL);

    let radius = CornerRadius::same(3);
    let widgets = &mut v.widgets;

    widgets.noninteractive.bg_fill = light::BACKDROP;
    widgets.noninteractive.weak_bg_fill = light::BACKDROP;
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, light::NORMAL);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, light::UNFOCUS);
    widgets.noninteractive.corner_radius = radius;

    widgets.inactive.bg_fill = light::SUBTLE;
    widgets.inactive.weak_bg_fill = light::FAINT;
    widgets.inactive.fg_stroke = Stroke::new(1.0, light::NORMAL);
    widgets.inactive.bg_stroke = Stroke::new(1.0, light::MINOR);
    widgets.inactive.corner_radius = radius;

    widgets.hovered.bg_fill = light::FAINT;
    widgets.hovered.weak_bg_fill = light::SUBTLE;
    widgets.hovered.fg_stroke = Stroke::new(1.0, light::IMPORTANT_GLOBAL);
    widgets.hovered.bg_stroke = Stroke::new(1.0, light::ACCENT);
    widgets.hovered.corner_radius = radius;

    widgets.active.bg_fill = light::SUBTLE;
    widgets.active.weak_bg_fill = light::FAINT;
    widgets.active.fg_stroke = Stroke::new(1.0, light::IMPORTANT_GLOBAL);
    widgets.active.bg_stroke = Stroke::new(1.0, light::ACCENT);
    widgets.active.corner_radius = radius;

    widgets.open.bg_fill = light::FAINT;
    widgets.open.fg_stroke = Stroke::new(1.0, light::IMPORTANT_GLOBAL);
    widgets.open.bg_stroke = Stroke::new(1.0, light::ACCENT);
    widgets.open.corner_radius = radius;

    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_visuals_panel_fill_matches_nugu_ui_backdrop() {
        let v = dark_visuals();
        assert_eq!(v.panel_fill, dark::BACKDROP);
        assert_eq!(v.window_fill, dark::BACKDROP);
    }

    #[test]
    fn light_visuals_panel_fill_matches_nugu_ui_backdrop() {
        let v = light_visuals();
        assert_eq!(v.panel_fill, light::BACKDROP);
        assert_eq!(v.window_fill, light::BACKDROP);
    }

    #[test]
    fn helper_colors_pick_per_mode() {
        assert_eq!(heading_color(ThemeMode::Dark), dark::IMPORTANT_GLOBAL);
        assert_eq!(heading_color(ThemeMode::Light), light::IMPORTANT_GLOBAL);
        assert_eq!(subtitle_color(ThemeMode::Dark), dark::IMPORTANT_LOCAL);
        assert_eq!(subtitle_color(ThemeMode::Light), light::IMPORTANT_LOCAL);
        assert_eq!(muted_color(ThemeMode::Dark), dark::MINOR);
        assert_eq!(muted_color(ThemeMode::Light), light::MINOR);
    }

    #[test]
    fn widget_text_contrasts_in_all_states() {
        for visuals in [dark_visuals(), light_visuals()] {
            let panel = visuals.panel_fill;
            for widget in [
                visuals.widgets.noninteractive,
                visuals.widgets.inactive,
                visuals.widgets.hovered,
                visuals.widgets.active,
                visuals.widgets.open,
            ] {
                let text = widget.fg_stroke.color;
                assert_contrast(text, panel);
                assert_contrast(text, widget.bg_fill);
                assert_contrast(text, widget.weak_bg_fill);
            }
        }
    }

    #[test]
    fn widget_state_strokes_keep_layout_stable() {
        for visuals in [dark_visuals(), light_visuals()] {
            let widgets = visuals.widgets;
            let width = widgets.inactive.bg_stroke.width;
            assert_eq!(widgets.hovered.bg_stroke.width, width);
            assert_eq!(widgets.active.bg_stroke.width, width);
            assert_eq!(widgets.open.bg_stroke.width, width);
        }
    }

    #[test]
    fn bundled_fonts_are_prepended_without_dropping_system_fallbacks() {
        let defaults = FontDefinitions::default();
        let fonts = font_definitions();
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let names = fonts.families.get(&family).expect("font family exists");
            assert_eq!(names[0], "jetbrains-mono-nf");
            assert_eq!(names[1], "noto-emoji");
            for default in defaults
                .families
                .get(&family)
                .expect("default font family exists")
                .iter()
                .filter(|name| *name != "jetbrains-mono-nf" && *name != "noto-emoji")
            {
                assert!(
                    names.contains(default),
                    "{family:?} should retain default fallback {default}"
                );
            }
        }
    }

    #[test]
    fn prepend_font_moves_existing_font_without_losing_fallbacks() {
        let mut fonts = FontDefinitions::default();
        let family = fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .expect("proportional family exists");
        family.insert(0, "existing".into());
        family.push("preferred".into());

        prepend_font(&mut fonts, FontFamily::Proportional, "preferred");

        let family = fonts
            .families
            .get(&FontFamily::Proportional)
            .expect("proportional family exists");
        assert_eq!(family[0], "preferred");
        assert_eq!(family.iter().filter(|name| *name == "preferred").count(), 1);
        assert!(family.iter().any(|name| name == "existing"));
    }

    fn assert_contrast(foreground: Color32, background: Color32) {
        const MIN_CONTRAST: f32 = 4.5;
        let contrast = contrast_ratio(foreground, background);
        assert!(
            contrast >= MIN_CONTRAST,
            "contrast {contrast:.2} below {MIN_CONTRAST} for fg={foreground:?} bg={background:?}",
        );
    }

    fn contrast_ratio(a: Color32, b: Color32) -> f32 {
        let a = relative_luminance(a);
        let b = relative_luminance(b);
        let (light, dark) = if a > b { (a, b) } else { (b, a) };
        (light + 0.05) / (dark + 0.05)
    }

    fn relative_luminance(color: Color32) -> f32 {
        let r = linear_channel(color.r());
        let g = linear_channel(color.g());
        let b = linear_channel(color.b());
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    fn linear_channel(value: u8) -> f32 {
        let value = f32::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
}
