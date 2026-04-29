use anyhow::{Context, Result};
use tracing::{debug, info};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuId, MenuItem},
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostQuitMessage, TranslateMessage,
};

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../assets/tray-icon.png");

/// Owns the tray icon and the IDs of its menu items so the event loop can
/// match incoming `MenuEvent`s.
pub struct Tray {
    // Held to keep the icon alive; dropping removes it from the tray.
    _icon: TrayIcon,
    quit_id: MenuId,
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
        _icon: tray,
        quit_id,
    })
}

/// Run the Win32 message loop on the calling thread. Blocks until a `Quit`
/// menu click posts `WM_QUIT`. Must be called from the same thread that
/// built the tray (tray-icon registers a hidden window on that thread).
///
/// # Safety
///
/// `GetMessageW`, `TranslateMessage`, `DispatchMessageW`, and
/// `PostQuitMessage` are FFI into the Win32 API. They are sound here
/// because we pass a properly initialized `MSG`, never reuse it across
/// threads, and only call `PostQuitMessage` in response to a real menu
/// event. The unsafe block is contained to the message-pump body.
pub fn run_event_loop(tray: &Tray) -> Result<()> {
    info!("entering tray message loop");
    let menu_rx = MenuEvent::receiver();
    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match result.0 {
            0 => break, // WM_QUIT — clean exit
            -1 => anyhow::bail!("GetMessageW failed"),
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }

        // Drain any menu events surfaced by the dispatch above.
        while let Ok(event) = menu_rx.try_recv() {
            debug!(id = ?event.id(), "menu event");
            if event.id() == &tray.quit_id {
                info!("Quit menu clicked");
                unsafe { PostQuitMessage(0) };
            }
        }
    }
    info!("tray message loop exited");
    Ok(())
}
