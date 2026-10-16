package com.limusic.player

import android.app.PendingIntent
import android.content.Intent
import androidx.annotation.OptIn
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * Keeps playback alive with the app in the background and owns the media notification. The
 * session wraps [PlayerHolder]'s player in a [ForwardingPlayer] that hands next/previous to the
 * app: ExoPlayer only ever holds the playing track and the one after it, and the real queue (with
 * shuffle, repeat and radio continuation) lives in Rust.
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {
  private var session: MediaSession? = null

  override fun onCreate() {
    super.onCreate()
    val player = QueueForwardingPlayer(PlayerHolder.get(this))
    val open = packageManager.getLaunchIntentForPackage(packageName)?.let {
      PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE)
    }
    session = MediaSession.Builder(this, player)
      .apply { if (open != null) setSessionActivity(open) }
      .build()
  }

  override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

  /** Swiped away from recents while paused: nothing to keep alive for. */
  override fun onTaskRemoved(rootIntent: Intent?) {
    val p = session?.player
    if (p == null || !p.playWhenReady || p.mediaItemCount == 0) {
      stopSelf()
    }
  }

  override fun onDestroy() {
    // The player itself stays: it belongs to the process, and the app may bind again.
    session?.release()
    session = null
    super.onDestroy()
  }
}

@OptIn(UnstableApi::class)
private class QueueForwardingPlayer(player: Player) : ForwardingPlayer(player) {
  private val queueCommands = listOf(
    Player.COMMAND_SEEK_TO_NEXT,
    Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
    Player.COMMAND_SEEK_TO_PREVIOUS,
    Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
  )

  override fun getAvailableCommands(): Player.Commands =
    super.getAvailableCommands().buildUpon().addAll(*queueCommands.toIntArray()).build()

  override fun isCommandAvailable(command: Int): Boolean =
    command in queueCommands || super.isCommandAvailable(command)

  override fun seekToNext() = NativeBridge.remote("next")

  override fun seekToNextMediaItem() = NativeBridge.remote("next")

  override fun seekToPrevious() = NativeBridge.remote("previous")

  override fun seekToPreviousMediaItem() = NativeBridge.remote("previous")

  /** Play with nothing loaded (the app restored its queue but never started it): let it start. */
  override fun play() {
    if (mediaItemCount == 0) NativeBridge.remote("play") else super.play()
  }
}
