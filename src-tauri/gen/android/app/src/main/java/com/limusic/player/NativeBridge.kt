package com.limusic.player

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.Log
import org.json.JSONObject

/**
 * Both directions between Rust (`src-tauri/src/android/mod.rs`) and the player, as static JNI
 * calls that need no activity: playback and the queue carry on with the app backgrounded or
 * swiped away.
 *
 * - [start] calls `register`, which hands Rust the JVM; from then on Rust calls [command] with one
 *   JSON object per player command (`crates/player/src/android.rs`) or app request.
 * - [emit] calls `onEvent`, which Rust turns into the same `PlayerEvent`s libmpv produces on
 *   desktop.
 */
object NativeBridge {
  private const val TAG = "LimusicPlayer"

  private lateinit var context: Context
  private var started = false

  @JvmStatic
  private external fun register()

  @JvmStatic
  private external fun onEvent(json: String)

  /** Open the command channel. Idempotent: the activity can be recreated, the bridge cannot. */
  fun start(ctx: Context) {
    context = ctx.applicationContext
    if (started) return
    started = true
    try {
      register()
    } catch (e: UnsatisfiedLinkError) {
      Log.e(TAG, "native side not loaded, the player has no commands", e)
    }
  }

  /** Called from Rust's player thread. Runs the command on ExoPlayer's looper, in order. */
  @JvmStatic
  fun command(json: String) {
    val args = JSONObject(json)
    PlayerHolder.main.post {
      try {
        run(args)
      } catch (e: Exception) {
        Log.e(TAG, "command ${args.optString("cmd")} failed", e)
      }
    }
  }

  private fun run(a: JSONObject) {
    val ctx = context
    when (a.getString("cmd")) {
      "load" -> PlayerHolder.load(
        ctx,
        a.getString("url"),
        a.optJSONObject("headers")?.let { h -> h.keys().asSequence().associateWith { h.getString(it) } }
          ?: emptyMap(),
        a.optDoubleOrNull("gainDb"),
        a.optDoubleOrNull("start"),
      )
      "enqueue" -> PlayerHolder.enqueue(ctx, a.getString("url"))
      "clearPlaylist" -> PlayerHolder.clearPlaylist(ctx)
      "stop" -> PlayerHolder.stop(ctx)
      "play" -> PlayerHolder.play(ctx)
      "pause" -> PlayerHolder.pause(ctx)
      "toggle" -> PlayerHolder.toggle(ctx)
      "seek" -> PlayerHolder.seek(ctx, a.getDouble("position"))
      "setVolume" -> PlayerHolder.setVolume(ctx, a.getDouble("volume"))
      "setLoop" -> PlayerHolder.setLoop(ctx, a.getBoolean("on"))
      "setSpeed" -> PlayerHolder.setSpeed(ctx, a.getDouble("speed"))
      "setPitch" -> PlayerHolder.setPitch(ctx, a.getInt("semitones"))
      "setGain" -> PlayerHolder.setGain(ctx, a.optDoubleOrNull("gainDb"))
      "setMetadata" -> PlayerHolder.setMetadata(
        ctx,
        a.optString("title"),
        a.optString("artist"),
        a.optStringOrNull("album"),
        a.optStringOrNull("artworkUrl"),
      )
      "openUrl" -> ctx.startActivity(
        Intent(Intent.ACTION_VIEW, Uri.parse(a.getString("url")))
          .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
      )
      "openLogin" -> ctx.startActivity(
        Intent(ctx, LoginActivity::class.java)
          .putExtra(LoginActivity.EXTRA_ADD_ACCOUNT, a.optBoolean("addAccount"))
          .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
      )
      else -> Log.w(TAG, "unknown command ${a.optString("cmd")}")
    }
  }

  fun emit(type: String, fill: JSONObject.() -> Unit = {}) {
    val json = JSONObject().put("type", type).apply(fill).toString()
    try {
      onEvent(json)
    } catch (e: UnsatisfiedLinkError) {
      // The service outlived the app's native side (or started without it): nothing to tell.
      Log.w(TAG, "native side not loaded, dropped $type")
    }
  }

  fun remote(action: String) = emit("remote") { put("action", action) }
}

private fun JSONObject.optDoubleOrNull(key: String): Double? =
  if (isNull(key)) null else optDouble(key).takeUnless { it.isNaN() }

private fun JSONObject.optStringOrNull(key: String): String? =
  if (isNull(key)) null else optString(key)
