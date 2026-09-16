package com.nearclip

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat

/**
 * Keeps the process (and with it the Rust listener and mDNS announcer, which
 * are created once per process) alive while the app is in the background, so
 * other devices can still send text. Also holds the multicast lock, without
 * which Android drops incoming mDNS packets.
 */
class NearclipService : Service() {
  private var multicastLock: WifiManager.MulticastLock? = null
  // The activity restarts us on every onCreate; only post the notification once.
  private var inForeground = false

  override fun onCreate() {
    super.onCreate()
    val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    multicastLock = wifi.createMulticastLock("nearclip-mdns").apply {
      setReferenceCounted(false)
      acquire()
    }
    val channel = NotificationChannel(
      CHANNEL_ID,
      "Background listening",
      NotificationManager.IMPORTANCE_LOW,
    ).apply { description = "Shown while nearclip can receive text in the background" }
    getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent?.action == ACTION_STOP) {
      // Pauses background listening until the app is next opened.
      stopSelf()
      return START_NOT_STICKY
    }
    if (!inForeground) {
      val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
      } else {
        0
      }
      ServiceCompat.startForeground(this, NOTIFICATION_ID, buildNotification(), type)
      inForeground = true
    }
    return START_STICKY
  }

  override fun onDestroy() {
    multicastLock?.let { if (it.isHeld) it.release() }
    multicastLock = null
    super.onDestroy()
  }

  override fun onBind(intent: Intent?): IBinder? = null

  private fun buildNotification(): Notification {
    val open = PendingIntent.getActivity(
      this,
      0,
      Intent(this, MainActivity::class.java),
      PendingIntent.FLAG_IMMUTABLE,
    )
    val stop = PendingIntent.getService(
      this,
      1,
      Intent(this, NearclipService::class.java).setAction(ACTION_STOP),
      PendingIntent.FLAG_IMMUTABLE,
    )
    return NotificationCompat.Builder(this, CHANNEL_ID)
      .setSmallIcon(R.mipmap.ic_launcher)
      .setContentTitle("nearclip")
      .setContentText("Listening for text from your devices")
      .setContentIntent(open)
      .addAction(0, "Stop", stop)
      .setOngoing(true)
      .setSilent(true)
      .build()
  }

  companion object {
    const val CHANNEL_ID = "nearclip.background"
    const val NOTIFICATION_ID = 1
    const val ACTION_STOP = "com.nearclip.action.STOP"

    fun start(context: Context) {
      ContextCompat.startForegroundService(context, Intent(context, NearclipService::class.java))
    }
  }
}
