use anyhow::{Context, Result};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuId, MenuItem},
};

const TRAY_ICON_ACTIVE_BYTES: &[u8] = include_bytes!("../assets/tray-icon.png");
const TRAY_ICON_PAUSED_BYTES: &[u8] = include_bytes!("../assets/tray-icon-paused.png");

/// Owns the tray icon and its menu IDs. The Win32 message pump that drives
/// menu / icon events lives in `eframe`'s winit event loop; we just keep
/// the tray alive for the lifetime of the app.
pub struct Tray {
    icon: TrayIcon,
    pub edit_id: MenuId,
    pub log_id: MenuId,
    pub quit_id: MenuId,
    active_icon: Icon,
    paused_icon: Icon,
}

fn decode_icon(bytes: &[u8]) -> Result<Icon> {
    let img = image::load_from_memory(bytes)
        .context("decoding tray icon")?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Icon::from_rgba(img.into_raw(), width, height).context("building tray Icon")
}

pub fn build() -> Result<Tray> {
    let active_icon = decode_icon(TRAY_ICON_ACTIVE_BYTES)?;
    let paused_icon = decode_icon(TRAY_ICON_PAUSED_BYTES)?;

    let menu = Menu::new();

    let edit = MenuItem::new("Edit Reminders", true, None);
    let edit_id = edit.id().clone();
    menu.append(&edit)
        .context("appending Edit Reminders menu item")?;

    let log = MenuItem::new("Open Logs Folder", true, None);
    let log_id = log.id().clone();
    menu.append(&log).context("appending Open Log menu item")?;

    let quit = MenuItem::new("Quit", true, None);
    let quit_id = quit.id().clone();
    menu.append(&quit).context("appending Quit menu item")?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("kree")
        .with_icon(active_icon.clone())
        .build()
        .context("building tray icon")?;

    Ok(Tray {
        icon: tray,
        edit_id,
        log_id,
        quit_id,
        active_icon,
        paused_icon,
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

    /// Swap to the active or paused icon variant. The crate's
    /// `set_icon` takes ownership, so we clone an `Icon` from the
    /// preloaded pair on every call — Icon is internally
    /// reference-counted, so the clone is cheap.
    pub fn set_paused(&self, paused: bool) -> Result<()> {
        let icon = if paused {
            &self.paused_icon
        } else {
            &self.active_icon
        };
        self.icon
            .set_icon(Some(icon.clone()))
            .context("set_icon failed")
    }
}
