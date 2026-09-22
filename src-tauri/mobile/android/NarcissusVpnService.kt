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
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.util.Log
import java.io.File
import java.io.IOException

class NarcissusVpnService : VpnService() {

    companion object {
        const val TAG = "NarcissusVpnService"
        const val ACTION_CONNECT = "com.narcissus.aura.VPN_CONNECT"
        const val ACTION_DISCONNECT = "com.narcissus.aura.VPN_DISCONNECT"
        const val ACTION_STANDBY = "com.narcissus.aura.VPN_STANDBY"
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

        fun startStandby(context: Context) {
            val intent = Intent(context, NarcissusVpnService::class.java).apply {
                action = ACTION_STANDBY
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
    @Volatile
    private var tunnelRequested = false
    // Reference to the app's visible Activity. The VPN-consent Intent MUST be
    // launched from it, not from the Service: a background service starting an
    // activity is blocked (or rendered as a translucent blank dialog) by
    // Android 10+ background-activity-start restrictions on many OEM ROMs.
    @Volatile
    private var currentActivity: Activity? = null
    // The system VPN-consent Intent, retained so the foreground notification can
    // offer it as a tappable fallback (notification taps are exempt from
    // background-activity-start restrictions on every OEM ROM).
    @Volatile
    private var pendingConsentIntent: Intent? = null
    private val mainHandler = Handler(Looper.getMainLooper())

    private val lifecycleCallbacks = object : Application.ActivityLifecycleCallbacks {
        override fun onActivityResumed(activity: Activity) {
            currentActivity = activity
            if (tunnelRequested && vpnInterface == null) {
                Log.i(TAG, "Activity resumed, retrying VPN establishment")
                tryEstablishTunnel()
            }
        }
        override fun onActivityCreated(activity: Activity, savedInstanceState: android.os.Bundle?) {}
        override fun onActivityStarted(activity: Activity) {}
        override fun onActivityPaused(activity: Activity) {}
        override fun onActivityStopped(activity: Activity) {
            if (currentActivity === activity) currentActivity = null
        }
        override fun onActivitySaveInstanceState(activity: Activity, outState: android.os.Bundle) {}
        override fun onActivityDestroyed(activity: Activity) {
            if (currentActivity === activity) currentActivity = null
        }
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
                tunnelRequested = true
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
            ACTION_STANDBY -> {
                isRunning = true
                try {
                    startForeground(NOTIFICATION_ID, buildNotification("待机中", "Narcissus Aura VPN 服务就绪"))
                } catch (e: Exception) {
                    Log.w(TAG, "startForeground failed (notification permission?): ${e.message}")
                }
                startPendingWatch()
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
     * Attempt to establish the VPN tunnel. Always marshalled onto the main
     * thread: the consent dialog must be started from a UI context, and calling
     * Activity.startActivity from the background watch thread is unreliable.
     */
    private fun tryEstablishTunnel() {
        mainHandler.post { doEstablish() }
    }

    private fun doEstablish() {
        // Reuse fast path: a live tunnel already exists (e.g. reconnect), just
        // hand its fd back to Rust which is polling for it.
        val existing = vpnInterface
        if (existing != null) {
            isTunReady = true
            writeTunFd(existing.fd)
            writeVpnStatus("established:${existing.fd}")
            return
        }

        try {
            val prepareIntent = prepare(this)
            if (prepareIntent != null) {
                pendingConsentIntent = prepareIntent
                writeVpnStatus("consent_required")
                val act = currentActivity
                if (act != null && !act.isFinishing) {
                    // Launch consent from the visible Activity on the main thread:
                    // exempt from background-activity-start limits, so the dialog
                    // renders correctly. onActivityResumed re-triggers once the
                    // user allows or denies.
                    Log.i(TAG, "VPN consent required, launching dialog from Activity (main thread)")
                    try {
                        act.startActivity(prepareIntent)
                        return
                    } catch (e: Exception) {
                        Log.w(TAG, "Activity launch failed, using notification fallback: ${e.message}")
                    }
                } else {
                    Log.i(TAG, "No foreground Activity, using notification fallback")
                }
                // Fallback: surface the consent as a tappable notification. A tap is
                // user-initiated, so it bypasses BAL on every ROM.
                showConsentNotification()
                return
            }

            writeVpnStatus("establishing")
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
                pendingConsentIntent = null
                writeTunFd(fd)
                writeVpnStatus("established:$fd")
                updateNotification("正在运行", "Narcissus Aura VPN 已连接")
                Log.i(TAG, "VPN TUN established: fd=$fd")
            } else {
                writeVpnStatus("establish_failed:null")
                Log.e(TAG, "builder.establish() returned null")
            }
        } catch (e: Exception) {
            writeVpnStatus("error:${e.javaClass.simpleName}:${e.message}")
            Log.e(TAG, "Failed to establish VPN TUN", e)
        }
    }

    private fun writeVpnStatus(status: String) {
        try {
            File(dataDir, "vpn_status").writeText(status)
            Log.i(TAG, "vpn_status -> $status")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to write vpn_status", e)
        }
    }

    private fun showConsentNotification() {
        val consent = pendingConsentIntent ?: return
        try {
            val pi = PendingIntent.getActivity(
                this, 100,
                Intent(consent).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M)
                    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
                else PendingIntent.FLAG_UPDATE_CURRENT
            )
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            val notification = Notification.Builder(this, CHANNEL_ID)
                .setContentTitle("需要允许 VPN 连接")
                .setContentText("Narcissus Aura 正在等待你的 VPN 权限授权，点击此处完成")
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setContentIntent(pi)
                .setOngoing(true)
                .setAutoCancel(false)
                .build()
            manager.notify(NOTIFICATION_ID + 1, notification)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to show consent notification", e)
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
            val pendingFile = File(dataDir, "vpn_pending")
            val stopFile = File(dataDir, "vpn_stop")
            while (isRunning && !Thread.currentThread().isInterrupted) {
                // Rust writes vpn_stop when the user disconnects. Close the tunnel
                // but keep the service alive so a later connect can re-hand it over.
                if (stopFile.exists()) {
                    try { stopFile.delete() } catch (_: Exception) {}
                    if (vpnInterface != null) {
                        Log.i(TAG, "Detected vpn_stop signal, closing tunnel")
                        cleanupVpnInterface()
                        writeVpnStatus("standby")
                        updateNotification("待机中", "Narcissus Aura VPN 服务就绪")
                    }
                    try {
                        (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager)
                            .cancel(NOTIFICATION_ID + 1)
                    } catch (_: Exception) {}
                    pendingConsentIntent = null
                }
                if (pendingFile.exists()) {
                    try { pendingFile.delete() } catch (_: Exception) {}
                    tunnelRequested = true
                    // doEstablish() reuses a live tunnel's fd or builds a new one,
                    // marshalled onto the main thread internally.
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
            val fdFile = File(dataDir, "tun_fd")
            fdFile.writeText(fd.toString())
            Log.i(TAG, "Wrote TUN fd $fd to ${fdFile.absolutePath}")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to write TUN fd", e)
        }
    }

    private fun shutdown() {
        isRunning = false
        isTunReady = false
        tunnelRequested = false
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
