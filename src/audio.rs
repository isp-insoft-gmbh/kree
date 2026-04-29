use std::io::Cursor;
use std::time::Duration;

use anyhow::Result;
use tracing::warn;

const CHIME_BYTES: &[u8] = include_bytes!("../assets/chime.ogg");

/// Chime length plus a safety margin. Generated chime is ~480 ms; we hold
/// the audio device open a touch longer so the tail isn't clipped.
const PLAYBACK_HOLD: Duration = Duration::from_millis(700);

/// Play the embedded chime once on a worker thread. Failures are logged
/// but never propagate — a missing audio device must not break reminders.
pub fn play_chime() {
    std::thread::spawn(|| {
        if let Err(e) = play_chime_inner() {
            warn!(error = %e, "chime playback failed");
        }
    });
}

fn play_chime_inner() -> Result<()> {
    let stream_handle = rodio::DeviceSinkBuilder::open_default_sink()?;
    let mixer = stream_handle.mixer();
    let _player = rodio::play(mixer, Cursor::new(CHIME_BYTES))?;
    // Block long enough for the chime to finish before the handle drops
    // and the device closes.
    std::thread::sleep(PLAYBACK_HOLD);
    Ok(())
}

/// Speak the given text aloud via the system TTS engine on a worker
/// thread. Same fire-and-forget posture as [`play_chime`]: failures are
/// logged but never propagate.
pub fn speak(text: String) {
    std::thread::spawn(move || {
        if let Err(e) = speak_inner(&text) {
            warn!(error = %e, text = %text, "TTS failed");
        }
    });
}

fn speak_inner(text: &str) -> Result<()> {
    let mut engine = tts::Tts::default()?;
    engine.speak(text, false)?;
    // Poll until SAPI is done — dropping the engine mid-utterance cancels
    // the speech, so we keep this worker thread parked until the engine
    // is idle.
    while engine.is_speaking()? {
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}
