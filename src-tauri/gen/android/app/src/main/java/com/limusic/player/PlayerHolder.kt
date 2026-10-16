package com.limusic.player

import android.content.Context
import android.media.AudioManager
import android.media.audiofx.LoudnessEnhancer
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.HttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import org.json.JSONObject
import kotlin.math.pow

/**
 * The one ExoPlayer, shared by [PlayerPlugin] (commands from Rust) and [PlaybackService] (the media
 * session, notification and headset buttons). Every method runs on the main looper, which is where
 * ExoPlayer wants to be called; [PlayerPlugin] posts there.
 *
 * Mirrors what `crates/player` expects of a backend: `Load` replaces the playlist without touching
 * the pause flag (mpv's `loadfile`), `Enqueue` appends the gapless lookahead, and a finished item is
 * reported as `ended` whether ExoPlayer moved on to the next one or ran out.
 */
@OptIn(UnstableApi::class)
object PlayerHolder {
  private const val TAG = "LimusicPlayer"
  private const val TICK_MS = 250L

  val main = Handler(Looper.getMainLooper())

  private var exo: ExoPlayer? = null
  private val http = DefaultHttpDataSource.Factory()
    .setAllowCrossProtocolRedirects(true)
    .setConnectTimeoutMs(15_000)
    .setReadTimeoutMs(20_000)

  private var userVolume = 1f
  private var gainDb: Double? = null
  private var speed = 1f
  private var pitch = 1f
  private var enhancer: LoudnessEnhancer? = null
  private var lastPlaying = false
  private var lastPaused = false
  private var lastDuration = C.TIME_UNSET

  fun get(context: Context): ExoPlayer {
    exo?.let { return it }
    val ctx = context.applicationContext
    val player = ExoPlayer.Builder(ctx)
      .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(ctx, http)))
      .setAudioAttributes(
        AudioAttributes.Builder()
          .setUsage(C.USAGE_MEDIA)
          .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
          .build(),
        /* handleAudioFocus = */ true,
      )
      .setHandleAudioBecomingNoisy(true)
      .setWakeMode(C.WAKE_MODE_NETWORK)
      .build()
    // A fixed session id, so the loudness enhancer can attach before the first track.
    player.audioSessionId =
      (ctx.getSystemService(Context.AUDIO_SERVICE) as AudioManager).generateAudioSessionId()
    // mpv starts unpaused; the app pauses explicitly when it means to.
    player.playWhenReady = true
    player.addListener(listener)
    exo = player
    return player
  }

  private fun emitState() {
    val p = exo ?: return
    val idle = p.mediaItemCount == 0 ||
      p.playbackState == Player.STATE_IDLE ||
      p.playbackState == Player.STATE_ENDED
    NativeBridge.emit("state") {
      put("idle", idle)
      put("current", p.currentMediaItem?.mediaId ?: JSONObject.NULL)
    }
  }

  /** mpv's "playing" is `!pause && !idle`: buffering mid-track still counts. */
  private fun updatePlaying() {
    val p = exo ?: return
    val now = p.playWhenReady &&
      (p.playbackState == Player.STATE_READY || p.playbackState == Player.STATE_BUFFERING)
    if (now != lastPlaying) {
      lastPlaying = now
      NativeBridge.emit("playing") { put("value", now) }
      main.removeCallbacks(tick)
      if (now) main.post(tick)
    }
  }

  private val tick = object : Runnable {
    override fun run() {
      val p = exo ?: return
      emitPosition(p)
      if (lastPlaying) main.postDelayed(this, TICK_MS)
    }
  }

  /**
   * mpv reports a duration per file. ExoPlayer's can arrive with READY, with a timeline update, or
   * already be known when a gapless hand-over lands on a preloaded item (no state change at all),
   * so every one of those asks, and only a new value goes out.
   */
  private fun maybeEmitDuration() {
    val d = exo?.duration ?: return
    if (d == C.TIME_UNSET || d == lastDuration) return
    lastDuration = d
    NativeBridge.emit("duration") { put("value", d / 1000.0) }
  }

  private fun emitPosition(p: Player) {
    NativeBridge.emit("position") { put("value", p.currentPosition / 1000.0) }
  }

  private val listener = object : Player.Listener {
    override fun onPlaybackStateChanged(state: Int) {
      emitState()
      if (state == Player.STATE_READY) maybeEmitDuration()
      if (state == Player.STATE_ENDED) NativeBridge.emit("ended")
      updatePlaying()
    }

    override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
      if (lastPaused != !playWhenReady) {
        lastPaused = !playWhenReady
        NativeBridge.emit("paused") { put("value", !playWhenReady) }
      }
      updatePlaying()
    }

    override fun onTimelineChanged(timeline: androidx.media3.common.Timeline, reason: Int) =
      maybeEmitDuration()

    override fun onMediaItemTransition(item: MediaItem?, reason: Int) {
      // AUTO is the gapless hand-over into the enqueued lookahead: the item before it ended.
      // REPEAT (repeat-one) never ended as far as the app is concerned, same as mpv's loop-file.
      if (reason == Player.MEDIA_ITEM_TRANSITION_REASON_AUTO) {
        emitState()
        NativeBridge.emit("ended")
      }
      // Then the new item's length, in mpv's order: end of the old file, duration of the new one.
      if (reason != Player.MEDIA_ITEM_TRANSITION_REASON_PLAYLIST_CHANGED) {
        lastDuration = C.TIME_UNSET
        maybeEmitDuration()
      }
    }

    override fun onPositionDiscontinuity(
      old: Player.PositionInfo,
      new: Player.PositionInfo,
      reason: Int,
    ) {
      if (reason == Player.DISCONTINUITY_REASON_SEEK) exo?.let { emitPosition(it) }
    }

    override fun onPlayerError(error: PlaybackException) {
      Log.w(TAG, "playback failed", error)
      emitState()
      NativeBridge.emit("failed") { put("message", friendly(error)) }
    }
  }

  /** What the user sees in the "skipped" toast; mirrors `friendly_error` in crates/player. */
  private fun friendly(e: PlaybackException): String {
    var cause: Throwable? = e
    while (cause != null) {
      if (cause is HttpDataSource.InvalidResponseCodeException) {
        return "The stream was refused (HTTP ${cause.responseCode})"
      }
      cause = cause.cause
    }
    return when (e.errorCode) {
      PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED,
      PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT -> "Network error while streaming"
      PlaybackException.ERROR_CODE_PARSING_CONTAINER_MALFORMED,
      PlaybackException.ERROR_CODE_DECODING_FAILED -> "This track could not be decoded"
      else -> e.errorCodeName
    }
  }

  // --- commands ------------------------------------------------------------------------------

  fun load(ctx: Context, url: String, headers: Map<String, String>, gain: Double?, start: Double?) {
    val p = get(ctx)
    applyHeaders(headers)
    gainDb = gain
    val item = MediaItem.Builder().setUri(url).setMediaId(url).build()
    lastDuration = C.TIME_UNSET
    p.setMediaItem(item, ((start ?: 0.0) * 1000).toLong())
    p.prepare()
    applyVolume()
    emitState()
  }

  fun enqueue(ctx: Context, url: String) {
    val p = get(ctx)
    // Items already played stay in ExoPlayer's playlist after a gapless hand-over; drop them so
    // it holds only what mpv's would: the playing track and what comes after it.
    if (p.currentMediaItemIndex > 0) p.removeMediaItems(0, p.currentMediaItemIndex)
    p.addMediaItem(MediaItem.Builder().setUri(url).setMediaId(url).build())
  }

  /** mpv's `playlist-clear`: everything but the playing item goes. */
  fun clearPlaylist(ctx: Context) {
    val p = get(ctx)
    val idx = p.currentMediaItemIndex
    if (p.mediaItemCount > idx + 1) p.removeMediaItems(idx + 1, p.mediaItemCount)
    if (idx > 0) p.removeMediaItems(0, idx)
  }

  fun stop(ctx: Context) {
    val p = get(ctx)
    p.stop()
    p.clearMediaItems()
    emitState()
  }

  fun play(ctx: Context) {
    val p = get(ctx)
    if (p.playbackState == Player.STATE_IDLE && p.mediaItemCount > 0) p.prepare()
    p.play()
  }

  fun pause(ctx: Context) = get(ctx).pause()

  fun toggle(ctx: Context) {
    val p = get(ctx)
    if (p.playWhenReady) p.pause() else play(ctx)
  }

  fun seek(ctx: Context, seconds: Double) = get(ctx).seekTo((seconds * 1000).toLong())

  fun setVolume(ctx: Context, volume: Double) {
    get(ctx)
    userVolume = volume.toFloat().coerceIn(0f, 1f)
    applyVolume()
  }

  fun setGain(ctx: Context, gain: Double?) {
    get(ctx)
    gainDb = gain
    applyVolume()
  }

  fun setLoop(ctx: Context, on: Boolean) {
    get(ctx).repeatMode = if (on) Player.REPEAT_MODE_ONE else Player.REPEAT_MODE_OFF
  }

  fun setSpeed(ctx: Context, value: Double) {
    speed = value.toFloat()
    get(ctx).playbackParameters = PlaybackParameters(speed, pitch)
  }

  fun setPitch(ctx: Context, semitones: Int) {
    pitch = 2.0.pow(semitones / 12.0).toFloat()
    get(ctx).playbackParameters = PlaybackParameters(speed, pitch)
  }

  /** What the notification, lock screen and car display show for the playing item. */
  fun setMetadata(ctx: Context, title: String, artist: String, album: String?, artwork: String?) {
    val p = get(ctx)
    val item = p.currentMediaItem ?: return
    val meta = MediaMetadata.Builder()
      .setTitle(title)
      .setArtist(artist)
      .setAlbumTitle(album)
      .setArtworkUri(artwork?.let { android.net.Uri.parse(it) })
      .build()
    p.replaceMediaItem(p.currentMediaItemIndex, item.buildUpon().setMediaMetadata(meta).build())
  }

  private fun applyHeaders(headers: Map<String, String>) {
    val ua = headers.entries.firstOrNull { it.key.equals("user-agent", true) }?.value
    http.setUserAgent(ua)
    http.setDefaultRequestProperties(headers.filterKeys { !it.equals("user-agent", true) })
  }

  /**
   * Loudness normalization. A cut is a plain volume scale; a boost goes through the platform's
   * [LoudnessEnhancer], since ExoPlayer's volume cannot go above 1.
   */
  private fun applyVolume() {
    val p = exo ?: return
    val g = gainDb ?: 0.0
    if (g <= 0.0) {
      p.volume = userVolume * 10.0.pow(g / 20.0).toFloat()
      enhancer?.enabled = false
      return
    }
    p.volume = userVolume
    try {
      val e = enhancer ?: LoudnessEnhancer(p.audioSessionId).also { enhancer = it }
      e.setTargetGain((g * 100).toInt())
      e.enabled = true
    } catch (e: RuntimeException) {
      Log.w(TAG, "loudness enhancer unavailable", e)
    }
  }
}
