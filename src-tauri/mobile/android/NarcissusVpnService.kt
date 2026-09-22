package com.narcissus.aura

import android.app.Activity
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
        const val CHANNEL_ID = "narcissus_vpn_channel"
        const val NOTIFICATION_ID = 1001

        @Volatile
        var isRunning = false
            private set

        @Volatile
        var isTunReady = false
            private set

        // Reference to the app's visible Activity, maintained by the global
        // lifecycle callbacks registered in VpnInitProvider. The VPN-consent
        // Intent MUST be launched from it, not from the Service: a background
        // service starting an activity is blocked (or rendered as a translucent
        // blank dialog) by Android 10+ background-activity-start restrictions
        // on many OEM ROMs.
        @Volatile
        var currentActivity: Activity? = null

        @Volatile
        private var instance: NarcissusVpnService? = null

        // Set only when we were forced to fall back to startForegroundService()
        // (i.e. the OS handed us a foreground-service promise we MUST honour).
        // startService() paths never set it, so a failing startForeground() there
        // can never trigger ForegroundServiceDidNotStartInTimeException.
        @Volatile
        private var fgsPromisePending = false

        fun notifyActivityResumed() {
            val svc = instance ?: return
            if (svc.tunnelRequested && svc.vpnInterface == null) {
                Log.i(TAG, "Activity resumed, retrying VPN establishment")
                svc.tryEstablishTunnel()
            }
        }

        // True while a tunnel request is outstanding and unfulfilled; gates the
        // provider's native consent button.
        fun tunnelPending(): Boolean {
            val svc = instance ?: return false
            return svc.tunnelRequested && svc.vpnInterface == null
        }

        fun startVpn(context: Context) {
            val intent = Intent(context, NarcissusVpnService::class.java).apply {
                action = ACTION_CONNECT
            }
            // Prefer plain startService: the user just tapped connect, so the app
            // is in the foreground and this is always allowed — and crucially it
            // creates NO foreground-service promise. v0.2.77 crashed with
            // ForegroundServiceDidNotStartInTimeException because startForeground
            // failed on MIUI (notifications blocked) while the promise from
            // startForegroundService was still pending; the system killed the
            // process 5s later.
            try {
                context.startService(intent)
            } catch (e: IllegalStateException) {
                Log.w(TAG, "startService rejected, falling back to FGS: ${e.message}")
                fgsPromisePending = true
                try {
                    context.startForegroundService(intent)
                } catch (e2: Exception) {
                    fgsPromisePending = false
                    throw e2
                }
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
    @Volatile
    private var tunnelRequested = false
    // True between "we raised the system consent dialog" and its dismissal, so
    // a resume-triggered re-entry can detect a user denial instead of looping.
    @Volatile
    private var consentDialogShown = false
    // The AUTO consent launch (a startActivity not bound to user input) is
    // silently dropped by MIUI's background-dialog policy: it resumes the
    // activity instantly with a cancel, which reads back as a denial. That
    // attempt must therefore happen exactly once per connect; afterwards the
    // input-bound native orange button is the reliable consent path.
    @Volatile
    private var consentAutoAttempted = false
    // The system VPN-consent Intent, retained so the foreground notification can
    // offer it as a tappable fallback (notification taps are exempt from
    // background-activity-start restrictions on every OEM ROM).
    @Volatile
    private var pendingConsentIntent: Intent? = null
    private val mainHandler = Handler(Looper.getMainLooper())

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        instance = this
    }

    /**
     * Calls startForeground() and NEVER lets the exception escape. Returns false
     * only when we held a startForegroundService() promise we could not keep —
     * the only case that would end in ForegroundServiceDidNotStartInTimeException,
     * so we stopSelf() before the 5s OS deadline (stopping a service cancels the
     * pending-promise timeout). When the service was started with plain
     * startService() a notification failure is cosmetic: the caller may proceed.
     */
    private fun resolveForegroundPromise(title: String, content: String): Boolean {
        val heldPromise = fgsPromisePending
        fgsPromisePending = false
        val ok = try {
            startForeground(NOTIFICATION_ID, buildNotification(title, content))
            true
        } catch (t: Throwable) {
            Log.e(TAG, "startForeground failed: ${t.javaClass.simpleName}: ${t.message}")
            false
        }
        if (!ok && heldPromise) {
            stopSelf()
            return false
        }
        return true
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_CONNECT -> {
                isRunning = true
                tunnelRequested = true
                // A fresh connect attempt earns a fresh dialog allowance (the
                // native overlay button stays available meanwhile).
                consentDialogShown = false
                consentAutoAttempted = false
                if (!resolveForegroundPromise("正在连接", "Narcissus Aura VPN 正在建立连接...")) {
                    // We hold a startForegroundService() promise we could not keep:
                    // stopSelf() already ran before the 5s deadline. The Rust side
                    // surfaces this stage in the UI.
                    writeVpnStatus("fgs_start_failed:notification_blocked")
                    return START_NOT_STICKY
                }
                tryEstablishTunnel()
            }
            ACTION_DISCONNECT -> {
                shutdown()
            }
            else -> {
                // Started without an action (e.g. OEM restart): just hold the
                // foreground slot. Tunnel requests arrive via ACTION_CONNECT
                // from the process-level watchdog in VpnInitProvider.
                isRunning = true
                resolveForegroundPromise("待机中", "Narcissus Aura VPN 服务就绪")
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
                if (consentDialogShown) {
                    // The dialog we raised already came and went but consent is
                    // still missing: the user denied (or MIUI instantly dropped
                    // it). Re-launching on every resume would trap them in an
                    // inescapable prompt loop.
                    consentDialogShown = false
                    writeVpnStatus("consent_denied")
                    return
                }
                if (consentAutoAttempted) {
                    // The single auto attempt is spent; stay in consent_denied
                    // so the native button remains available. Rust keeps the
                    // connect alive meanwhile and the button tap re-enters here
                    // with consent already granted.
                    writeVpnStatus("consent_denied")
                    return
                }
                consentAutoAttempted = true
                pendingConsentIntent = prepareIntent
                writeVpnStatus("consent_required")
                val act = currentActivity
                if (act != null && !act.isFinishing) {
                    // Launch consent from the visible Activity on the main thread:
                    // exempt from background-activity-start limits, so the dialog
                    // renders correctly. onActivityResumed re-triggers once the
                    // user allows or denies. NEW_TASK makes MIUI treat it as a
                    // normal app-initiated cross-package launch.
                    Log.i(TAG, "VPN consent required, launching dialog from Activity (main thread)")
                    try {
                        act.startActivity(Intent(prepareIntent).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                        consentDialogShown = true
                        writeVpnStatus("consent_dialog_opened")
                        return
                    } catch (e: Exception) {
                        Log.w(TAG, "Activity launch failed, using notification fallback: ${e.message}")
                        writeVpnStatus("consent_activity_failed:${e.javaClass.simpleName}")
                    }
                } else {
                    Log.i(TAG, "No foreground Activity, using notification fallback")
                    writeVpnStatus("consent_no_activity")
                }
                // Fallback: surface the consent as a tappable notification. A tap is
                // user-initiated, so it bypasses BAL on every ROM.
                showConsentNotification()
                return
            }
            consentDialogShown = false

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
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && !manager.areNotificationsEnabled()) {
                // notify() would silently no-op: tell the UI so the user can
                // enable notifications instead of waiting for a dialog forever.
                writeVpnStatus("consent_notify_blocked")
                return
            }
            val pi = PendingIntent.getActivity(
                this, 100,
                Intent(consent).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M)
                    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
                else PendingIntent.FLAG_UPDATE_CURRENT
            )
            val notification = Notification.Builder(this, CHANNEL_ID)
                .setContentTitle("需要允许 VPN 连接")
                .setContentText("Narcissus Aura 正在等待你的 VPN 权限授权，点击此处完成")
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setContentIntent(pi)
                .setOngoing(true)
                .setAutoCancel(false)
                .build()
            manager.notify(NOTIFICATION_ID + 1, notification)
            writeVpnStatus("consent_notify_posted")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to show consent notification", e)
            writeVpnStatus("consent_notify_failed:${e.javaClass.simpleName}")
        }
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
        consentDialogShown = false
        cleanupVpnInterface()
        pendingConsentIntent = null
        try {
            (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager)
                .cancel(NOTIFICATION_ID + 1)
        } catch (_: Exception) {}
        writeVpnStatus("standby")
        instance = null
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
        instance = null
        cleanupVpnInterface()
        try { stopForeground(true) } catch (_: Exception) {}
        stopSelf()
        super.onDestroy()
    }

    private fun createNotificationChannel() {
        try {
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
        } catch (t: Throwable) {
            Log.e(TAG, "createNotificationChannel failed: ${t.message}")
        }
    }

    private fun updateNotification(title: String, content: String) {
        try {
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            manager.notify(NOTIFICATION_ID, buildNotification(title, content))
        } catch (_: Exception) {}
    }

    // Must never throw: this runs inside the startForeground() call that has a
    // pending FGS promise on some ROMs, and an exception here previously led to
    // a swallowed failure and a 5s process kill.
    private fun buildNotification(title: String, content: String): Notification {
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }

        builder
            .setContentTitle(title)
            .setContentText(content)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setOngoing(true)

        try {
            val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
            if (launchIntent != null) {
                val pendingIntent = PendingIntent.getActivity(
                    this, 0, launchIntent,
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) PendingIntent.FLAG_IMMUTABLE else 0
                )
                builder.setContentIntent(pendingIntent)
            }
        } catch (t: Throwable) {
            Log.w(TAG, "Notification contentIntent unavailable: ${t.message}")
        }

        return builder.build()
    }
}
