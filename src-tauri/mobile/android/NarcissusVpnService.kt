package com.narcissus.aura

import android.app.Activity
import android.app.Application
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log
import java.io.File
import java.io.IOException

class NarcissusVpnService : VpnService() {

    companion object {
        const val TAG = "NarcissusVpnService"
        const val ACTION_CONNECT = "com.narcissus.aura.VPN_CONNECT"
        const val ACTION_DISCONNECT = "com.narcissus.aura.VPN_DISCONNECT"
        const val CHANNEL_ID = "narcissus_vpn_channel"
        const val NOTIFICATION_ID = 1001

        @Volatile
        var isRunning = false
            private set

        @Volatile
        var isTunReady = false
            private set

        fun startVpn(context: Context) {
            val intent = Intent(context, NarcissusVpnService::class.java).apply {
                action = ACTION_CONNECT
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stopVpn(context: Context) {
            isTunReady = false
            val intent = Intent(context, NarcissusVpnService::class.java).apply {
                action = ACTION_DISCONNECT
            }
            context.startService(intent)
        }
    }

    private var vpnInterface: ParcelFileDescriptor? = null
    private var lifecycleRegistered = false
    private var watchThread: Thread? = null

    private val lifecycleCallbacks = object : Application.ActivityLifecycleCallbacks {
        override fun onActivityResumed(activity: Activity) {
            if (isRunning && vpnInterface == null) {
                Log.i(TAG, "Activity resumed, retrying VPN establishment")
                tryEstablishTunnel()
            }
        }
        override fun onActivityCreated(activity: Activity, savedInstanceState: android.os.Bundle?) {}
        override fun onActivityStarted(activity: Activity) {}
        override fun onActivityPaused(activity: Activity) {}
        override fun onActivityStopped(activity: Activity) {}
        override fun onActivitySaveInstanceState(activity: Activity, outState: android.os.Bundle) {}
        override fun onActivityDestroyed(activity: Activity) {}
    }

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        application.registerActivityLifecycleCallbacks(lifecycleCallbacks)
        lifecycleRegistered = true
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_CONNECT -> {
                isRunning = true
                try {
                    startForeground(NOTIFICATION_ID, buildNotification("正在连接", "Narcissus Aura VPN 正在建立连接..."))
                } catch (e: Exception) {
                    Log.w(TAG, "startForeground failed (notification permission?): ${e.message}")
                }
                tryEstablishTunnel()
                startPendingWatch()
            }
            ACTION_DISCONNECT -> {
                shutdown()
            }
            else -> {
                // No action specified (e.g. restart from ContentProvider)
                isRunning = true
                try {
                    startForeground(NOTIFICATION_ID, buildNotification("待机中", "Narcissus Aura VPN 服务就绪"))
                } catch (e: Exception) {
                    Log.w(TAG, "startForeground failed (notification permission?): ${e.message}")
                }
                startPendingWatch()
            }
        }
        return START_NOT_STICKY
    }

    /**
     * Attempt to establish the VPN tunnel immediately.
     * If user consent is required (prepare() returns Intent), launches the consent dialog.
     */
    private fun tryEstablishTunnel() {
        if (vpnInterface != null) return

        try {
            val prepareIntent = prepare(this)
            if (prepareIntent != null) {
                Log.i(TAG, "VPN consent required, launching system dialog")
                prepareIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                startActivity(prepareIntent)
                return
            }

            val builder = Builder()
                .setSession("Narcissus Aura")
                .setMtu(9000)
                .addAddress("172.19.0.1", 30)
                .addDnsServer("1.1.1.1")
                .addDnsServer("8.8.8.8")
                .addRoute("0.0.0.0", 0)

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                try {
                    builder.addDisallowedApplication(packageName)
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to exclude own package: ${e.message}")
                }
            }

            vpnInterface = builder.establish()
            if (vpnInterface != null) {
                val fd = vpnInterface!!.fd
                isTunReady = true
                writeTunFd(fd)
                updateNotification("正在运行", "Narcissus Aura VPN 已连接")
                Log.i(TAG, "VPN TUN established: fd=$fd")
            } else {
                Log.e(TAG, "builder.establish() returned null")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to establish VPN TUN", e)
        }
    }

    /**
     * Background thread that watches for the vpn_pending signal file from Rust.
     * When Rust is ready to connect (in TUN mode), it writes vpn_pending.
     * This thread detects it and triggers tunnel establishment.
     */
    private fun startPendingWatch() {
        if (watchThread?.isAlive == true) return
        watchThread = Thread({
            val pendingFile = File(filesDir, "vpn_pending")
            while (isRunning && !Thread.currentThread().isInterrupted) {
                if (pendingFile.exists() && vpnInterface == null) {
                    Log.i(TAG, "Detected vpn_pending signal, establishing tunnel")
                    try {
                        pendingFile.delete()
                    } catch (_: Exception) {}
                    tryEstablishTunnel()
                }
                try {
                    Thread.sleep(500)
                } catch (_: InterruptedException) {
                    break
                }
            }
        }, "vpn-pending-watch")
        watchThread!!.isDaemon = true
        watchThread!!.start()
    }

    private fun writeTunFd(fd: Int) {
        try {
            val fdFile = File(filesDir, "tun_fd")
            fdFile.writeText(fd.toString())
            Log.i(TAG, "Wrote TUN fd $fd to ${fdFile.absolutePath}")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to write TUN fd", e)
        }
    }

    private fun shutdown() {
        isRunning = false
        isTunReady = false
        watchThread?.interrupt()
        watchThread = null
        cleanupVpnInterface()
        if (lifecycleRegistered) {
            try {
                application.unregisterActivityLifecycleCallbacks(lifecycleCallbacks)
            } catch (_: Exception) {}
            lifecycleRegistered = false
        }
        try { stopForeground(true) } catch (_: Exception) {}
        stopSelf()
    }

    private fun cleanupVpnInterface() {
        try {
            vpnInterface?.close()
        } catch (e: IOException) {
            Log.e(TAG, "Error closing vpn interface", e)
        }
        vpnInterface = null
        isTunReady = false
    }

    override fun onDestroy() {
        isRunning = false
        isTunReady = false
        watchThread?.interrupt()
        cleanupVpnInterface()
        if (lifecycleRegistered) {
            try {
                application.unregisterActivityLifecycleCallbacks(lifecycleCallbacks)
            } catch (_: Exception) {}
        }
        try { stopForeground(true) } catch (_: Exception) {}
        stopSelf()
        super.onDestroy()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Narcissus VPN Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Narcissus Aura 代理隧道连接状态"
            }
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            manager.createNotificationChannel(channel)
        }
    }

    private fun updateNotification(title: String, content: String) {
        try {
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            manager.notify(NOTIFICATION_ID, buildNotification(title, content))
        } catch (_: Exception) {}
    }

    private fun buildNotification(title: String, content: String): Notification {
        val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
        val pendingIntent = PendingIntent.getActivity(
            this, 0, launchIntent,
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) PendingIntent.FLAG_IMMUTABLE else 0
        )

        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }

        return builder
            .setContentTitle(title)
            .setContentText(content)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .build()
    }
}
