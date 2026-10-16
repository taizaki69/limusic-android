//! What a player backend reports back to the app. Shared by the mpv (desktop) and Android backends.

/// Events a backend pumps to the app (mpv's event thread, or the Android bridge). context/14.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Position(f64),
    Duration(f64),
    /// Playback started or stopped, emitted only on a real change.
    ///
    /// Derived from mpv's `pause` **and** `idle-active`, because `pause` alone is a trap: it starts
    /// out `false` and a `loadfile` doesn't touch it, so starting a track sets `false` → `false`
    /// and fires **no** property event at all. `idle-active` is the one that actually flips when a
    /// file starts (and when the playlist runs dry). Anything reading playback state off `pause`
    /// alone never hears that a track began, and only recovers on a manual pause/unpause.
    Playing(bool),
    /// mpv's `pause` flag changed: someone pressed pause or play. Unlike [`PlayerEvent::Playing`]
    /// this says nothing when the playlist runs dry or a new file starts, which is what a Listen
    /// Together host has to broadcast: a track change is announced on its own, and a "stopped"
    /// from the gap between two tracks reaches the room after the next one has started.
    Paused(bool),
    /// One track finished normally (EOF) — orchestrator advances the queue.
    TrackEnded,
    /// One track died (end-file with error, e.g. its URL 403'd). mpv may have auto-advanced
    /// into the next playlist entry or gone idle — the orchestrator asks [`Player::is_idle`].
    TrackFailed(String),
    /// mpv could not open the audio device, so the file ended with an error that has nothing to do
    /// with the stream: the device went away under it (sleep/resume, unplugged, a driver restart).
    /// Kept apart from [`PlayerEvent::TrackFailed`] because the two want opposite handling, see
    /// `AppState::on_audio_device_lost`.
    AudioDeviceLost,
    /// The crossfade preload deck failed to open the *next* track (dead or 403 URL). Kept apart
    /// from [`PlayerEvent::TrackFailed`] because reporting it as that would skip the track the
    /// user is hearing; what is owed is evicting the next track's cached URL, see
    /// `AppState::on_lookahead_failed`.
    LookaheadFailed(String),
    /// mpv could not open the music video for the audio file it names (exactly as mpv was handed
    /// it). The sound is unaffected; the app decides whether another stream for the picture is
    /// worth trying. The open is a network round trip, so a skip or a crossfade can land first,
    /// and the file named is then no longer the one playing.
    VideoFailed(String),
    Error(String),
}
