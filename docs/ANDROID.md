# Limusic on Android

The Android app is the same Limusic: the same Rust core (InnerTube client, stream resolution,
queue, radio, library, lyrics, scrobbling, Listen Together, SQLite database) and the same Svelte UI,
packaged with Tauri 2's mobile support. What changes is the layer that touches the operating
system.

## How it fits together

```
 Svelte UI (WebView) ──invoke──▶ Rust core (src-tauri, crates/*)
                                   │
                                   │ player::Command (JSON, JNI)     ▲ PlayerEvent (JSON, JNI)
                                   ▼                                 │
                    NativeBridge.kt ──▶ PlayerHolder.kt (ExoPlayer) ──┘
                                              │
                                   PlaybackService.kt (MediaSessionService)
                                   notification · lock screen · headset · Bluetooth · Android Auto
```

| Desktop                                  | Android                                                        |
| ---------------------------------------- | -------------------------------------------------------------- |
| libmpv (`crates/player/src/mpv`)          | Media3 ExoPlayer (`crates/player/src/android.rs` + Kotlin)      |
| MPRIS / SMTC via souvlaki (`media.rs`)    | Media3 `MediaSession` in a foreground service                   |
| Login window (`session.rs`)               | `LoginActivity.kt`, a WebView whose cookies go to the same `sign_in` |
| Tray, mini player, global hotkeys         | none: the media notification is the way back in                 |
| Custom titlebar + sidebar                 | top app bar + bottom navigation (`MobileNav.svelte`)            |
| Desktop player bar                        | `MobilePlayerBar.svelte`: a strip that expands with the player view |
| BotGuard PoToken (V8) and cipher webview  | not built: streams come from the direct-URL clients and rustypipe |
| Self-updater                              | none: install a new APK                                         |

The player bridge is plain JNI in both directions, on static methods of `NativeBridge.kt`, so it
needs no activity: playback and queue advancement carry on after the app is backed out of or
swiped away. (`PlayerPlugin.kt`, a Tauri plugin, only binds the media service and opens the
bridge when the activity starts.)

- **Rust → Kotlin**: `player::Player` on Android serializes each call as a `Command`
  (`{"cmd": "load", "url": ..., "gainDb": ..., "start": ...}`). A dedicated thread in
  `src-tauri/src/android/mod.rs`, attached to the JVM, calls `NativeBridge.command(json)`, which
  posts it to ExoPlayer's looper. Nothing ever waits on the JVM.
- **Kotlin → Rust**: `NativeBridge.emit` calls `Java_com_limusic_player_NativeBridge_onEvent`
  with `{"type": "position" | "duration" | "playing" | "paused" | "ended" | "failed" | "state" |
  "remote" | "loginCookie" | "loginError", ...}`. These become the same `PlayerEvent`s mpv produces,
  so `state.rs` and the event pump in `lib.rs` do not know which backend is playing.

ExoPlayer only ever holds the playing track and the gapless lookahead; the queue (shuffle, repeat,
radio continuation) stays in Rust. The notification's next/previous buttons go back to Rust as
`remote` events.

## Building

You need the Android SDK (platform 37, build-tools 37), an NDK (29 is what CI uses), JDK 21, Rust
with the Android targets, Node 22 + pnpm 10, and libclang on the build machine (bindgen generates
QuickJS bindings for rustypipe at build time).

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
source scripts/android-env.sh          # finds the SDK/NDK, sets the bindgen sysroot
cd ui && pnpm install

pnpm tauri android dev                 # run on a connected device or emulator
pnpm tauri android build --apk --target aarch64            # release APK, arm64
pnpm tauri android build --apk --debug --target aarch64    # debug APK
```

APKs land in `src-tauri/gen/android/app/build/outputs/apk/`.

Release signing: put a `keystore.properties` next to `src-tauri/gen/android/settings.gradle`:

```properties
storeFile=release.jks
storePassword=...
keyAlias=...
keyPassword=...
```

Without one, release builds are signed with the debug key so they still install. CI does the same
from the `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and
`ANDROID_KEY_PASSWORD` secrets (`.github/workflows/android.yml`).

## What is not there (yet)

- **Music videos** and **crossfade**: both lean on mpv (its video output and its second deck).
  The settings are hidden; playback stays gapless.
- **Japanese romanized lyrics** read kana only. The IPADIC dictionary is ~45 MB, too much for an
  APK; kanji lines stay as written.
- **PoToken-gated clients** (WEB_REMIX streams, TVHTML5_SIMPLY): there is no V8 build for
  `rustypipe-botguard` on Android and no hidden webview for the cipher. Streams come from VISIONOS
  and the rustypipe fallback, as they do on desktop when those runtimes fail to start.
- **Session refresh**: desktop reloads music.youtube.com in a hidden webview to rotate a rejected
  cookie. On Android you sign in again.
- **Local music**: the folder picker returns `content://` URIs, which the scanner does not read.
- **Discord Rich Presence**, the **tray**, the **mini player**, **global hotkeys**, the
  **self-updater**: desktop concepts with no Android counterpart.
