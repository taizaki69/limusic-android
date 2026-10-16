//! The Android half of the app: the bridge to the Kotlin player
//! (`gen/android/app/src/main/java/com/limusic/player/`).
//!
//! Both directions are plain JNI calls on static methods of `NativeBridge`, so neither depends on
//! the activity: the player service keeps playing, and the queue keeps advancing, after the user
//! backs out of the app or swipes it away. (Tauri's own plugin calls dispatch through the
//! activity's context, which is why commands do not use them.)
//!
//! - **Kotlin → Rust**: `NativeBridge.register()` once, when the Kotlin side is up, hands over the
//!   JVM and the class ([`Java_com_limusic_player_NativeBridge_register`]); after that every player
//!   event is `NativeBridge.onEvent(json)` ([`Java_com_limusic_player_NativeBridge_onEvent`]),
//!   parsed into the same [`PlayerEvent`]s the mpv backend produces, so everything above the player
//!   (state.rs, the event pump in lib.rs) runs unchanged.
//! - **Rust → Kotlin**: one dedicated thread, attached to the JVM, calls
//!   `NativeBridge.command(json)` for each queued command. The player and the media handle only push
//!   onto its channel, so no caller (a tokio task, Tauri's main thread) waits on the JVM, and
//!   commands arrive in the order they were sent. Kotlin posts them to ExoPlayer's looper.
//!
//! The Tauri plugin below exists only to load `PlayerPlugin.kt` with the activity, which binds the
//! media session service and then calls `register`.

pub mod media;

use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, OnceLock};

use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use player::PlayerEvent;
use serde::Deserialize;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Emitter, Manager, Wry};

use crate::state::{AppState, SignInOutcome};

static OUTBOX: OnceLock<Sender<serde_json::Value>> = OnceLock::new();
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

/// The plugin that loads the Kotlin side.
pub fn init() -> TauriPlugin<Wry> {
    Builder::new("limusic-player")
        .setup(|app, api| {
            api.register_android_plugin("com.limusic.player", "PlayerPlugin")?;
            let _ = APP.set(app.clone());
            Ok(())
        })
        .build()
}

/// Start the outbound thread and open the player's path to it. Runs once, from `register`.
fn start_outbox(vm: JavaVM, class: GlobalRef) {
    let (tx, rx) = channel::<serde_json::Value>();
    if OUTBOX.set(tx).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("android-player-out".into())
        .spawn(move || {
            let mut env = match vm.attach_current_thread_permanently() {
                Ok(env) => env,
                Err(e) => {
                    tracing::error!(error = %e, "android player thread could not attach to the JVM");
                    return;
                }
            };
            let class = <&JClass>::from(class.as_obj());
            while let Ok(payload) = rx.recv() {
                if let Err(e) = call_command(&mut env, class, &payload.to_string()) {
                    tracing::warn!(error = %e, "android player refused a command");
                    // A pending Java exception would fail every later call on this thread.
                    let _ = env.exception_describe();
                    let _ = env.exception_clear();
                }
            }
        })
        .expect("spawn android player thread");
    player::set_android_bridge(|cmd| match serde_json::to_value(&cmd) {
        Ok(v) => send(v),
        Err(e) => tracing::error!(error = %e, "player command did not serialize"),
    });
}

fn call_command(env: &mut JNIEnv, class: &JClass, json: &str) -> jni::errors::Result<()> {
    // A local frame per call: this thread lives as long as the app, so local refs must not pile up.
    env.with_local_frame(4, |env| {
        let s = env.new_string(json)?;
        env.call_static_method(
            class,
            "command",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&JObject::from(s))],
        )?;
        Ok(())
    })
}

/// Queue one `{"cmd": ...}` for the Kotlin side. Dropped (with a log line) before it is up; the
/// player crate keeps its own backlog for that window, so only media metadata can be lost, and the
/// next track change resends it.
pub(crate) fn send(payload: serde_json::Value) {
    match OUTBOX.get() {
        Some(tx) => {
            let _ = tx.send(payload);
        }
        None => tracing::debug!("android player not up yet, dropped a command"),
    }
}

/// What `PlayerPlugin.kt` reports, as `{"type": ..., ...}`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum Event {
    Position {
        value: f64,
    },
    Duration {
        value: f64,
    },
    Playing {
        value: bool,
    },
    Paused {
        value: bool,
    },
    /// One item finished: ExoPlayer moved on to the enqueued one, or ran out.
    Ended,
    Failed {
        message: String,
    },
    /// After every change that matters for `is_idle`/`current_path`; sent before `ended`.
    State {
        idle: bool,
        current: Option<String>,
    },
    /// The notification, a headset or Android Auto asked for something only the app's queue knows.
    Remote {
        action: String,
    },
    /// LoginActivity landed on music.youtube.com signed in; these are its cookies.
    LoginCookie {
        cookie: String,
    },
    /// LoginActivity was closed before signing in, or its WebView failed.
    LoginError {
        message: String,
    },
}

fn on_event(json: &str) {
    let event: Event = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, json, "unreadable event from the android player");
            return;
        }
    };
    let out = match event {
        Event::Position { value } => PlayerEvent::Position(value),
        Event::Duration { value } => PlayerEvent::Duration(value),
        Event::Playing { value } => PlayerEvent::Playing(value),
        Event::Paused { value } => PlayerEvent::Paused(value),
        Event::Ended => PlayerEvent::TrackEnded,
        Event::Failed { message } => PlayerEvent::TrackFailed(message),
        Event::State { idle, current } => {
            player::android_set_state(idle, current);
            return;
        }
        Event::Remote { action } => {
            remote(action);
            return;
        }
        Event::LoginCookie { cookie } => {
            login_cookie(cookie);
            return;
        }
        Event::LoginError { message } => {
            if let Some(app) = APP.get() {
                let _ = app.emit("login-error", message);
            }
            return;
        }
    };
    player::android_emit(out);
}

/// Same `AppState` paths as the desktop's MPRIS/SMTC handler (media.rs `handle_event`).
fn remote(action: String) {
    let Some(app) = APP.get().cloned() else { return };
    tauri::async_runtime::spawn(async move {
        let Some(state) = app.try_state::<Arc<AppState>>() else { return };
        let state = state.inner().clone();
        match action.as_str() {
            "next" => state.next_in_queue().await,
            "previous" => state.prev_in_queue().await,
            // Play from the notification after the app restored a queue but loaded nothing yet.
            "play" => state.resume_or_toggle().await,
            other => tracing::debug!(action = other, "unhandled remote action"),
        }
    });
}

/// Start the native sign-in. `add_account` makes LoginActivity clear the WebView's Google cookies
/// first, so the new sign-in is the only session in it (desktop does the same, session.rs).
pub(crate) fn open_login(_app: tauri::AppHandle, add_account: bool) {
    send(serde_json::json!({ "cmd": "openLogin", "addAccount": add_account }));
}

/// The same tail as the desktop login window's (session.rs `open_login`).
fn login_cookie(cookie: String) {
    let Some(app) = APP.get().cloned() else { return };
    tauri::async_runtime::spawn(async move {
        let Some(state) = app.try_state::<Arc<AppState>>() else { return };
        let state = state.inner().clone();
        if innertube::cookie_sapisid(&cookie).is_none() {
            let _ = app.emit("login-error", "Sign-in did not finish, try again".to_string());
            return;
        }
        match state.sign_in(cookie, None).await {
            Ok(SignInOutcome::Complete) => {
                let _ = app.emit("login-done", ());
            }
            Ok(SignInOutcome::SelectionRequired) => {}
            Err(e) => {
                let _ = app.emit("login-error", e);
            }
        }
    });
}

/// `com.limusic.player.NativeBridge.register()`: the Kotlin side is ready for commands.
#[no_mangle]
pub extern "system" fn Java_com_limusic_player_NativeBridge_register(env: JNIEnv, class: JClass) {
    let (Ok(vm), Ok(class)) = (env.get_java_vm(), env.new_global_ref(class)) else {
        tracing::error!("android player could not keep a reference to its JVM");
        return;
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || start_outbox(vm, class)));
}

/// `com.limusic.player.NativeBridge.onEvent(String)`. Never panics across the FFI boundary: a
/// panic here would abort the whole app.
#[no_mangle]
pub extern "system" fn Java_com_limusic_player_NativeBridge_onEvent(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    let json: String = match env.get_string(&json) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    let _ = std::panic::catch_unwind(|| on_event(&json));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_parse() {
        let e: Event = serde_json::from_str(r#"{"type":"position","value":1.5}"#).unwrap();
        assert!(matches!(e, Event::Position { value } if value == 1.5));
        let e: Event =
            serde_json::from_str(r#"{"type":"state","idle":false,"current":"http://x"}"#).unwrap();
        assert!(matches!(e, Event::State { idle: false, current: Some(_) }));
        let e: Event = serde_json::from_str(r#"{"type":"ended"}"#).unwrap();
        assert!(matches!(e, Event::Ended));
    }
}
