//! The audio player. YouTube-agnostic: takes a fully-resolved URL + headers, never a videoId.
//!
//! Two backends behind one API:
//! - desktop: libmpv ([`mpv`]), gapless through mpv's own playlist, crossfade on a second deck;
//! - Android: Media3/ExoPlayer, running in a foreground `MediaSessionService` on the Kotlin side
//!   ([`android`]). The app installs the bridge to it at startup with [`set_android_bridge`].

mod event;
pub use event::PlayerEvent;

#[cfg(not(target_os = "android"))]
mod mpv;
#[cfg(not(target_os = "android"))]
pub use mpv::*;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::*;
