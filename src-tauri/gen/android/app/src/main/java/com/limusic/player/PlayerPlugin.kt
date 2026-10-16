package com.limusic.player

import android.app.Activity
import android.content.ComponentName
import android.webkit.WebView
import androidx.appcompat.app.AppCompatActivity
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Plugin
import com.google.common.util.concurrent.ListenableFuture

/**
 * Loaded by `src-tauri/src/android/mod.rs` with the activity. It binds [PlaybackService] and opens
 * the [NativeBridge]; commands and events then flow through the bridge, not through this plugin, so
 * they keep working once the activity is gone.
 */
@TauriPlugin
class PlayerPlugin(private val activity: Activity) : Plugin(activity) {
  private var controller: ListenableFuture<MediaController>? = null

  override fun load(webView: WebView) {
    super.load(webView)
    // A controller binds PlaybackService, which is what lets Media3 promote it to a foreground
    // service (and show the notification) as soon as playback starts.
    val token = SessionToken(activity, ComponentName(activity, PlaybackService::class.java))
    controller = MediaController.Builder(activity, token).buildAsync()
    NativeBridge.start(activity)
  }

  override fun onDestroy(activity: AppCompatActivity) {
    controller?.let { MediaController.releaseFuture(it) }
    controller = null
    super.onDestroy(activity)
  }
}
