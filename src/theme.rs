//! kree's egui palettes, mapped from the user's nugu themes
//! (`~/Workspaces/dotfiles/nugu/output/{dark,light}.toml`, generated
//! by `just build` from `nugu.nu` + `palette.nu`).
//!
//! `apply` is idempotent — `App::ui` re-asserts visuals every frame,
//! and every popup viewport re-applies it inside its closure (each
//! viewport owns its own egui `Context`).

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Visuals,
};
use tracing::{info, warn};

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
    pub const FOCUS: Color32 = Color32::from_rgb(0x15, 0x33, 0x3b);
    pub const UNFOCUS: Color32 = Color32::from_rgb(0x5d, 0x5d, 0x5d);
    pub const IMPORTANT_LOCAL: Color32 = Color32::from_rgb(0x60, 0x49, 0x80);
    pub const IMPORTANT_GLOBAL: Color32 = Color32::from_rgb(0x3e, 0x2f, 0x53);

    pub const FAINT: Color32 = Color32::from_rgb(0xea, 0xea, 0xea);
    pub const SUBTLE: Color32 = Color32::from_rgb(0xcd, 0xcd, 0xcd);
}

const JETBRAINS_MONO_NF: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf");

/// Default chain of fonts loaded by absolute path from the Windows
/// fonts directory, used as the *secondary* fallback when the user
/// hasn't configured something explicit. Always present on Win10/11.
const DEFAULT_PATH_CANDIDATES: &[(&str, &str)] = &[
    ("segoe-ui-variable", r"C:\Windows\Fonts\SegUIVar.ttf"),
    ("segoe-ui", r"C:\Windows\Fonts\segoeui.ttf"),
    ("cascadia-mono", r"C:\Windows\Fonts\CascadiaMono.ttf"),
    ("cascadia-code", r"C:\Windows\Fonts\CascadiaCode.ttf"),
    ("segoe-emoji", r"C:\Windows\Fonts\seguiemj.ttf"),
    ("segoe-symbol", r"C:\Windows\Fonts\seguisym.ttf"),
    ("segoe-icons", r"C:\Windows\Fonts\SegoeIcons.ttf"),
];

/// Register fonts with egui:
///
/// 1. The bundled JetBrains Mono Nerd Font (always present).
/// 2. User-configured families from `config.font.proportional` /
///    `monospace` / `fallbacks`, resolved against the system font
///    index. Missing names are skipped with a warning — the rest of
///    the chain absorbs the gap.
/// 3. The hard-coded Segoe UI / Cascadia / Segoe Emoji chain by
///    absolute path as the final fallback.
///
/// Call once on startup and again on every `config.font.*` change.
/// `set_fonts` rebuilds the egui font atlas — non-trivial, but reloads
/// are user-driven, not per frame.
pub fn install_fonts(ctx: &egui::Context, config: &crate::config::Config) {
    let mut fonts = FontDefinitions::default();

    // Bundled — always present.
    fonts.font_data.insert(
        "jetbrains-mono-nf".into(),
        Arc::new(FontData::from_static(JETBRAINS_MONO_NF)),
    );

    // 1. User-configured families.
    let mut user_proportional: Option<String> = None;
    let mut user_monospace: Option<String> = None;
    let mut user_fallbacks: Vec<String> = Vec::new();

    if let Some(name) =
        register_user_family(&mut fonts, &config.font.proportional, "user-proportional")
    {
        user_proportional = Some(name);
    }
    if let Some(name) = register_user_family(&mut fonts, &config.font.monospace, "user-monospace") {
        user_monospace = Some(name);
    }
    for (i, family) in config.font.fallbacks.iter().enumerate() {
        let id = format!("user-fallback-{i}");
        if let Some(name) = register_user_family(&mut fonts, family, &id) {
            user_fallbacks.push(name);
        }
    }

    // 2. Hard-coded path candidates as the secondary fallback.
    let mut loaded_paths = vec!["jetbrains-mono-nf"];
    for (name, path) in DEFAULT_PATH_CANDIDATES {
        match std::fs::read(path) {
            Ok(bytes) => {
                fonts
                    .font_data
                    .insert((*name).into(), Arc::new(FontData::from_owned(bytes)));
                loaded_paths.push(*name);
            }
            Err(e) => warn!(font = name, path, error = %e, "system font missing; skipping"),
        }
    }

    info!(
        user_proportional = ?user_proportional,
        user_monospace = ?user_monospace,
        user_fallbacks = ?user_fallbacks,
        loaded_paths = ?loaded_paths,
        "fonts loaded"
    );

    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        if let Some(name) = &user_proportional {
            family.insert(0, name.clone());
        }
        for primary in ["segoe-ui-variable", "segoe-ui"] {
            if fonts.font_data.contains_key(primary) {
                family.push(primary.into());
            }
        }
        for fallback in &user_fallbacks {
            family.push(fallback.clone());
        }
        for fallback in [
            "jetbrains-mono-nf",
            "segoe-icons",
            "segoe-symbol",
            "segoe-emoji",
        ] {
            if fonts.font_data.contains_key(fallback) {
                family.push(fallback.into());
            }
        }
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        if let Some(name) = &user_monospace {
            family.insert(0, name.clone());
        }
        family.push("jetbrains-mono-nf".into());
        for fallback in ["cascadia-mono", "cascadia-code"] {
            if fonts.font_data.contains_key(fallback) {
                family.push(fallback.into());
            }
        }
        for fallback in &user_fallbacks {
            family.push(fallback.clone());
        }
        for fallback in ["segoe-icons", "segoe-symbol", "segoe-emoji"] {
            if fonts.font_data.contains_key(fallback) {
                family.push(fallback.into());
            }
        }
    }

    ctx.set_fonts(fonts);
}

/// Resolve a configured family name against the system font index, load
/// its bytes, and register them under `font_id` in `fonts`. Returns
/// `Some(font_id)` on success, `None` if the family was empty or
/// unresolvable. The caller appends `font_id` into the appropriate
/// family chain.
fn register_user_family(
    fonts: &mut FontDefinitions,
    family: &str,
    font_id: &str,
) -> Option<String> {
    if family.is_empty() {
        return None;
    }
    let path = match crate::fonts::resolve_family(family) {
        Some(p) => p,
        None => {
            warn!(family, "configured font family not found in system index");
            return None;
        }
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            warn!(family, path = %path.display(), error = %e, "could not read configured font");
            return None;
        }
    };
    fonts
        .font_data
        .insert(font_id.into(), Arc::new(FontData::from_owned(bytes)));
    Some(font_id.to_string())
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

    widgets.inactive.bg_fill = dark::UNFOCUS;
    widgets.inactive.weak_bg_fill = dark::FAINT;
    widgets.inactive.fg_stroke = Stroke::new(1.0, dark::NORMAL);
    widgets.inactive.bg_stroke = Stroke::new(1.0, dark::MINOR);
    widgets.inactive.corner_radius = radius;

    widgets.hovered.bg_fill = dark::ACCENT;
    widgets.hovered.weak_bg_fill = dark::FOCUS;
    widgets.hovered.fg_stroke = Stroke::new(1.5, dark::IMPORTANT_GLOBAL);
    widgets.hovered.bg_stroke = Stroke::new(1.5, dark::IMPORTANT_LOCAL);
    widgets.hovered.corner_radius = radius;

    widgets.active.bg_fill = dark::IMPORTANT_LOCAL;
    widgets.active.weak_bg_fill = dark::ACCENT;
    widgets.active.fg_stroke = Stroke::new(1.5, dark::IMPORTANT_GLOBAL);
    widgets.active.bg_stroke = Stroke::new(1.5, dark::IMPORTANT_GLOBAL);
    widgets.active.corner_radius = radius;

    widgets.open.bg_fill = dark::FOCUS;
    widgets.open.fg_stroke = Stroke::new(1.0, dark::IMPORTANT_GLOBAL);
    widgets.open.bg_stroke = Stroke::new(1.0, dark::ACCENT);
    widgets.open.corner_radius = radius;

    let _ = dark::SUBTLE; // silence unused-const warning until we use it
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

    widgets.hovered.bg_fill = light::ACCENT;
    widgets.hovered.weak_bg_fill = light::FOCUS;
    widgets.hovered.fg_stroke = Stroke::new(1.5, light::BACKDROP);
    widgets.hovered.bg_stroke = Stroke::new(1.5, light::IMPORTANT_LOCAL);
    widgets.hovered.corner_radius = radius;

    widgets.active.bg_fill = light::IMPORTANT_LOCAL;
    widgets.active.weak_bg_fill = light::ACCENT;
    widgets.active.fg_stroke = Stroke::new(1.5, light::BACKDROP);
    widgets.active.bg_stroke = Stroke::new(1.5, light::IMPORTANT_GLOBAL);
    widgets.active.corner_radius = radius;

    widgets.open.bg_fill = light::FOCUS;
    widgets.open.fg_stroke = Stroke::new(1.0, light::BACKDROP);
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
}
