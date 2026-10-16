//! Android backend: the same [`Player`] surface as the mpv one, played by Media3/ExoPlayer.
//!
//! ExoPlayer lives on the Kotlin side, inside a `MediaSessionService` (so the system keeps the
//! process alive in the background, shows the media notification and routes headset buttons). This
//! crate does not know how commands get there: the app installs a bridge with
//! [`set_android_bridge`] that forwards each [`Command`] to its Tauri mobile plugin, and feeds what
//! the Kotlin player reports back in through [`android_emit`] and [`android_set_state`].
//!
//! Every method here is fire-and-forget and never blocks on the Kotlin side: the orchestrator calls
//! the player from tokio tasks and from Tauri's main thread alike, and ExoPlayer only accepts calls
//! on its own looper, so a synchronous round trip from the wrong thread would deadlock. What the
//! app reads back synchronously (`is_idle`, `current_path`, `volume`) is mirrored here, updated
//! optimistically by the command that changes it and corrected by the next state report.
//!
//! What mpv does that ExoPlayer does not: crossfade (accepted and ignored, playback stays gapless)
//! and music videos (the picture APIs are inert; the app hides the video toggle on Android).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::PlayerEvent;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The Kotlin player is gone (the service was killed and not yet rebuilt).
    #[error("the Android player is not available: {0}")]
    Unavailable(String),
    #[error("Pitch shifting isn't available in this build")]
    NoPitchFilter,
}

/// One instruction for the Kotlin player. Serialized as `{"cmd": "load", ...}` in camelCase, which
/// is what `PlayerPlugin.kt` reads.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    /// Replace the playlist with `url` and play it, starting at `start` seconds.
    Load {
        url: String,
        headers: HashMap<String, String>,
        gain_db: Option<f64>,
        start: Option<f64>,
    },
    /// Append the gapless lookahead behind the playing item.
    Enqueue {
        url: String,
    },
    /// Drop every item after the playing one.
    ClearPlaylist,
    Stop,
    Play,
    Pause,
    Toggle,
    Seek {
        position: f64,
    },
    /// Linear amplitude, 0.0 to 1.0 (already mapped from the slider's perceptual percent).
    SetVolume {
        volume: f64,
    },
    SetLoop {
        on: bool,
    },
    SetSpeed {
        speed: f64,
    },
    SetPitch {
        semitones: i32,
    },
    SetGain {
        gain_db: Option<f64>,
    },
}

type Bridge = Box<dyn Fn(Command) + Send + Sync>;

struct Shared {
    bridge: Option<Bridge>,
    /// Commands sent before the bridge was installed (the player is built in `setup`, the plugin
    /// a moment later), replayed in order on install.
    backlog: Vec<Command>,
    tx: Option<UnboundedSender<PlayerEvent>>,
    current: Option<String>,
}

static SHARED: Mutex<Shared> =
    Mutex::new(Shared { bridge: None, backlog: Vec::new(), tx: None, current: None });
static IDLE: AtomicBool = AtomicBool::new(true);
static VOLUME: AtomicI64 = AtomicI64::new(100);

/// Install the path to the Kotlin player. Replays whatever was sent before it existed.
pub fn set_android_bridge(bridge: impl Fn(Command) + Send + Sync + 'static) {
    let mut s = SHARED.lock().unwrap();
    for cmd in std::mem::take(&mut s.backlog) {
        bridge(cmd);
    }
    s.bridge = Some(Box::new(bridge));
}

/// An event from the Kotlin player, on its way to the app's event pump.
pub fn android_emit(event: PlayerEvent) {
    if let Some(tx) = &SHARED.lock().unwrap().tx {
        let _ = tx.send(event);
    }
}

/// The Kotlin player's state after a change: whether it has nothing to play, and the URL of the
/// item it is on.
pub fn android_set_state(idle: bool, current: Option<String>) {
    IDLE.store(idle, Ordering::SeqCst);
    SHARED.lock().unwrap().current = current;
}

fn send(cmd: Command) {
    let mut s = SHARED.lock().unwrap();
    match &s.bridge {
        Some(bridge) => bridge(cmd),
        None => s.backlog.push(cmd),
    }
}

/// The slider's percent to a linear amplitude: the same curve the desktop build hears, which is
/// mpv's perceptual mapping followed by mpv's own cubic volume law.
fn perceptual_to_linear(percent: i64) -> f64 {
    if percent <= 0 {
        return 0.0;
    }
    let mpv = 10f64.powf(-(1.0 - percent.min(100) as f64 / 100.0).powf(1.5));
    mpv.powi(3)
}

pub struct Player {
    events: Option<UnboundedReceiver<PlayerEvent>>,
}

impl Player {
    /// `cache_dir` is unused: ExoPlayer keeps its own buffer and the app's audio proxy caches.
    pub fn new(_cache_dir: &str) -> Result<Self, Error> {
        let (tx, rx) = unbounded_channel();
        SHARED.lock().unwrap().tx = Some(tx);
        Ok(Player { events: Some(rx) })
    }

    pub fn take_events(&mut self) -> Option<UnboundedReceiver<PlayerEvent>> {
        self.events.take()
    }

    pub fn load(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        gain_db: Option<f64>,
        start: Option<f64>,
    ) -> Result<(), Error> {
        IDLE.store(false, Ordering::SeqCst);
        SHARED.lock().unwrap().current = Some(url.to_owned());
        send(Command::Load { url: url.to_owned(), headers: headers.clone(), gain_db, start });
        Ok(())
    }

    pub fn enqueue(&self, url: &str) -> Result<(), Error> {
        send(Command::Enqueue { url: url.to_owned() });
        Ok(())
    }

    /// Crossfade needs two decoders playing at once; ExoPlayer's playlist is gapless instead.
    pub fn set_crossfade(&self, secs: Option<f64>) {
        if secs.is_some_and(|s| s >= 1.0) {
            tracing::info!("crossfade is not available on Android, playback stays gapless");
        }
    }

    pub fn clear_playlist(&self) -> Result<(), Error> {
        send(Command::ClearPlaylist);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), Error> {
        IDLE.store(true, Ordering::SeqCst);
        SHARED.lock().unwrap().current = None;
        send(Command::Stop);
        Ok(())
    }

    pub fn is_idle(&self) -> bool {
        IDLE.load(Ordering::SeqCst)
    }

    pub fn play(&self) -> Result<(), Error> {
        send(Command::Play);
        Ok(())
    }

    pub fn pause(&self) -> Result<(), Error> {
        send(Command::Pause);
        Ok(())
    }

    pub fn toggle(&self) -> Result<(), Error> {
        send(Command::Toggle);
        Ok(())
    }

    pub fn set_loop_file(&self, on: bool) -> Result<(), Error> {
        send(Command::SetLoop { on });
        Ok(())
    }

    pub fn seek(&self, position_secs: f64) -> Result<(), Error> {
        send(Command::Seek { position: position_secs.max(0.0) });
        Ok(())
    }

    pub fn set_volume(&self, volume: i64) -> Result<(), Error> {
        VOLUME.store(volume, Ordering::Relaxed);
        send(Command::SetVolume { volume: perceptual_to_linear(volume) });
        Ok(())
    }

    pub fn volume(&self) -> i64 {
        VOLUME.load(Ordering::Relaxed)
    }

    /// ExoPlayer only ever reads from the app's loopback audio proxy, which carries the proxy
    /// setting itself, so there is nothing to tell it.
    pub fn set_http_proxy(&self, _proxy: Option<&str>) -> Result<(), Error> {
        Ok(())
    }

    pub fn set_gain(&self, gain_db: Option<f64>) -> Result<(), Error> {
        send(Command::SetGain { gain_db });
        Ok(())
    }

    pub fn set_speed(&self, speed: f64) -> Result<(), Error> {
        send(Command::SetSpeed { speed: speed.clamp(0.25, 2.0) });
        Ok(())
    }

    pub fn set_pitch(&self, semitones: i32) -> Result<(), Error> {
        send(Command::SetPitch { semitones: semitones.clamp(-12, 12) });
        Ok(())
    }

    pub fn current_path(&self) -> Option<String> {
        SHARED.lock().unwrap().current.clone()
    }

    // Music videos: not on Android. These keep the app's calls compiling and doing nothing.

    pub fn set_video_for(&self, _audio_url: &str, _video_url: &str) {}

    pub fn set_video_visible(&self, _on: bool) {}

    pub fn video_output_up(&self) -> bool {
        false
    }

    pub fn video_thumbnail(&self, _max_width: usize, _unless_at: Option<f64>) -> Option<Thumbnail> {
        None
    }
}

/// Same shape as the desktop one; never produced on Android.
pub struct Thumbnail {
    pub pts: f64,
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_curve_ends() {
        assert_eq!(perceptual_to_linear(0), 0.0);
        assert!((perceptual_to_linear(100) - 1.0).abs() < 1e-9);
        assert!(perceptual_to_linear(50) < 0.5);
    }

    #[test]
    fn commands_serialize_camel_case() {
        let json = serde_json::to_string(&Command::Load {
            url: "http://127.0.0.1/x".into(),
            headers: HashMap::new(),
            gain_db: Some(-3.0),
            start: None,
        })
        .unwrap();
        assert!(json.contains(r#""cmd":"load""#), "{json}");
        assert!(json.contains(r#""gainDb":-3.0"#), "{json}");
        let json = serde_json::to_string(&Command::ClearPlaylist).unwrap();
        assert_eq!(json, r#"{"cmd":"clearPlaylist"}"#);
    }
}
