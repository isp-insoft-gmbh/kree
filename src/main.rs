use anyhow::Result;
use single_instance::SingleInstance;

fn main() -> Result<()> {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string());
    let mutex_name = format!("kree-singleton-{user}");

    // Bind to a local so the mutex stays held for the lifetime of the process.
    let instance = SingleInstance::new(&mutex_name)?;
    if !instance.is_single() {
        eprintln!("kree: another instance is already running; exiting.");
        return Ok(());
    }

    println!("kree");
    Ok(())
}
