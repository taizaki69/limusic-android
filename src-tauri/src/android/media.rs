//! Android's [`MediaHandle`]: what the media notification, the lock screen and Bluetooth displays
//! show. The session itself is the Kotlin player service's, so this only hands it the track's
//! metadata; play state, position and duration it already reads off ExoPlayer.

use std::sync::Mutex;

use tauri::AppHandle;

#[derive(Default, Clone)]
struct Meta {
    title: String,
    artist: String,
    album: Option<String>,
    cover: Option<String>,
}

pub struct MediaHandle {
    meta: Mutex<Meta>,
}

impl MediaHandle {
    pub fn set_metadata(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        cover: Option<&str>,
    ) {
        let meta = Meta {
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: album.map(str::to_owned),
            cover: cover.map(str::to_owned),
        };
        *self.meta.lock().unwrap() = meta.clone();
        push(&meta);
    }

    pub fn set_album(&self, album: &str) {
        let meta = {
            let mut m = self.meta.lock().unwrap();
            m.album = Some(album.to_owned());
            m.clone()
        };
        push(&meta);
    }

    pub fn set_duration(&self, _secs: f64) {}

    pub fn set_playback(&self, _playing: bool, _pos: f64) {}

    /// The phone's volume keys move the system's media stream, not the app's slider.
    pub fn set_volume(&self, _volume: i64) {}
}

fn push(m: &Meta) {
    super::send(serde_json::json!({
        "cmd": "setMetadata",
        "title": m.title,
        "artist": m.artist,
        "album": m.album,
        "artworkUrl": m.cover,
    }));
}

pub fn spawn(_app: AppHandle) -> Option<MediaHandle> {
    Some(MediaHandle { meta: Mutex::new(Meta::default()) })
}
