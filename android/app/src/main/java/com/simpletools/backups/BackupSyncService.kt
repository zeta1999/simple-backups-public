package com.simpletools.backups

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat

class BackupSyncService : Service() {

    companion object {
        const val TAG = "BackupSyncService"
        const val CHANNEL_ID = "simple_backups_sync"
        const val NOTIFICATION_ID = 1
        @Volatile var isRunning = false
    }

    private var workerThread: Thread? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIFICATION_ID, buildNotification("Starting…"))
        isRunning = true

        workerThread = Thread {
            try {
                val prefs = getSharedPreferences("settings", MODE_PRIVATE)
                val peerAddr = prefs.getString("peer_addr", "")
                    ?: prefs.getString("proxy_addr", "")
                    ?: ""
                if (peerAddr.isNotEmpty()) {
                    BackupNative.setPeerAddr(peerAddr)
                }
                val watchDirs = prefs.getString("watch_dirs", "") ?: ""
                if (watchDirs.isNotEmpty()) {
                    BackupNative.setWatchDirs(watchDirs)
                }

                updateNotification("Connecting to desktop…")
                BackupNative.connect()
                updateNotification("Watching (stub — native push pending)")
                while (isRunning && !Thread.currentThread().isInterrupted) {
                    Thread.sleep(5000)
                }
            } catch (e: InterruptedException) {
                Log.i(TAG, "Worker interrupted")
            } catch (e: Exception) {
                Log.e(TAG, "Service error: ${e.message}", e)
                updateNotification("Error: ${e.message}")
            }
        }
        workerThread?.start()

        return START_STICKY
    }

    override fun onDestroy() {
        isRunning = false
        workerThread?.interrupt()
        try {
            BackupNative.disconnect()
        } catch (e: Exception) {
            Log.e(TAG, "Cleanup error: ${e.message}")
        }
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            "simple-backups sync",
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "Background backup sync status"
        }
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(channel)
    }

    private fun buildNotification(text: String): Notification {
        val pendingIntent = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("simple-backups")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_menu_upload)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .build()
    }

    private fun updateNotification(text: String) {
        val manager = getSystemService(NotificationManager::class.java)
        manager.notify(NOTIFICATION_ID, buildNotification(text))
    }
}
