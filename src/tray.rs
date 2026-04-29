use anyhow::{Context, Result};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuId, MenuItem},
};

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../assets/tray-icon.png");

/// Owns the tray icon and its menu IDs. The Win32 message pump that drives
/// menu / icon events lives in `eframe`'s winit event loop; we just keep
/// the tray alive for the lifetime of the app.
pub struct Tray {
    icon: TrayIcon,
    pub quit_id: MenuId,
}

pub fn build() -> Result<Tray> {
    let img = image::load_from_memory(TRAY_ICON_BYTES)
        .context("decoding embedded tray icon")?
        .into_rgba8();
    let (width, height) = img.dimensions();
    let icon = Icon::from_rgba(img.into_raw(), width, height).context("building tray Icon")?;

    let menu = Menu::new();
    let quit = MenuItem::new("Quit", true, None);
    let quit_id = quit.id().clone();
    menu.append(&quit).context("appending Quit menu item")?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("kree")
        .with_icon(icon)
        .build()
        .context("building tray icon")?;

    Ok(Tray {
        icon: tray,
        quit_id,
    })
}

impl Tray {
    /// Top-center of the tray icon's screen rect, in physical pixels, if
    /// known. The popup is anchored so its bottom edge sits just above
    /// this point.
    pub fn rect_anchor(&self) -> Option<(f32, f32)> {
        let r = self.icon.rect()?;
        let cx = r.position.x as f32 + r.size.width as f32 / 2.0;
        let top = r.position.y as f32;
        Some((cx, top))
    }
}
