use anyhow::{Context, Result};
use auto_launch::{AutoLaunch, AutoLaunchBuilder};

const APP_NAME: &str = "kree";

/// Argv passed to the autostart entry. The `--silent` flag is purely a
/// documentation hint at this point — `main_visible` is already `false`
/// on every cold start — but registering it makes the user-visible
/// `HKCU\...\Run` value say `kree.exe --silent`, which matches the
/// intent and keeps the door open for divergent silent vs interactive
/// behavior later.
const AUTOSTART_ARGS: &[&str] = &["--silent"];

fn manager() -> Result<AutoLaunch> {
    let exe = std::env::current_exe().context("resolving current_exe for autostart")?;
    let exe_str = exe
        .to_str()
        .context("current_exe path is not valid UTF-8")?;
    AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(exe_str)
        .set_args(AUTOSTART_ARGS)
        .build()
        .context("building AutoLaunch")
}

pub fn is_enabled() -> Result<bool> {
    manager()?.is_enabled().context("querying autostart state")
}

pub fn set(enabled: bool) -> Result<()> {
    let m = manager()?;
    if enabled {
        m.enable().context("enabling autostart")
    } else {
        m.disable().context("disabling autostart")
    }
}
