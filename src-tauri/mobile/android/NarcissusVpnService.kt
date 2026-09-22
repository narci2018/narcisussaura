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
        // lifecycle callbacks registered in VpnInitProvider. The watchdog in
        // VpnInitProvider launches the VPN-consent dialog from it BEFORE the
        // service is ever started (the only sequence that works on stock
        // Android and MIUI); the service itself never launches activities.
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
    // The system VPN-consent Intent, retained so the foreground notification can
    // offer it as a tappable fallback (notification taps are exempt from
    // background-activity-start restrictions on every OEM ROM).
    @Volatile
    private var pendingConsentIntent: Intent? = null
    // Dedup: the Rust side re-signals vpn_pending every ~4s (MIUI kills the
    // service), and each re-entry would otherwise re-notify.
    @Volatile
    private var consentNotifyPosted = false
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
                // A watchdog re-signal (Rust re-writes vpn_pending every ~4s so
                // a MIUI-killed service comes back) re-enters here with
                // tunnelRequested already true. Only a genuinely fresh attempt
                // re-arms the consent notification.
                if (!tunnelRequested) {
                    consentNotifyPosted = false
                }
                tunnelRequested = true
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
     * thread: Builder.establish() and the notification bookkeeping are
     * cheapest and least surprising there, and re-entry from
     * notifyActivityResumed arrives on the Activity's thread.
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
                // The watchdog in VpnInitProvider owns dialog launching
                // (v2rayNG ordering: foreground Activity +
                // startActivityForResult, before the service exists).
                // Reaching here means consent is still missing: wait via the
                // tappable consent notification + Settings guidance. Rust
                // keeps re-signalling vpn_pending, and the
                // notifyActivityResumed path builds the tunnel the moment
                // consent exists — no further tap in this app needed.
                enterWaitingConsent(prepareIntent)
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
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                // v2rayNG does this too: MIUI otherwise flags the tunnel as a
                // metered network and can throttle/deprioritise it.
                builder.setMetered(false)
            }

            vpnInterface = builder.establish()
            if (vpnInterface != null) {
                val fd = vpnInterface!!.fd
                isTunReady = true
                pendingConsentIntent = null
                consentNotifyPosted = false
                try {
                    (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager)
                        .cancel(NOTIFICATION_ID + 1)
                } catch (_: Exception) {}
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

    /**
     * Consent is missing when the service runs. The watchdog already made the
     * v2rayNG-style launch attempts (foreground Activity +
     * startActivityForResult) BEFORE starting us, so this is the waiting
     * state: a tappable consent notification (tap = user-initiated, exempt
     * from background-activity-start rules on every ROM) plus the
     * 系统设置 → VPN → Narcissus Aura guidance stage, where SETTINGS itself
     * raises the dialog. Rust keeps re-signalling vpn_pending, so the moment
     * consent exists the next re-entry takes the prepare()==null path and
     * builds the tunnel without any further user action inside this app.
     */
    private fun enterWaitingConsent(prepareIntent: Intent) {
        pendingConsentIntent = prepareIntent
        writeVpnStatus("consent_denied")
        showConsentNotification()
        // Re-post the ongoing FGS notification too: buildNotification now
        // binds its tap to the consent intent, so the persistent
        // "connecting" notification is itself a consent entry point.
        updateNotification("需要授权", "Narcissus Aura 等待你点击此处完成 VPN 授权")
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
        if (consentNotifyPosted) return
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
            consentNotifyPosted = true
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
        cleanupVpnInterface()
        pendingConsentIntent = null
        consentNotifyPosted = false
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
            // While consent is pending, tapping the ongoing "connecting"
            // notification opens the consent page directly: another tap
            // target that no dialog-blocking policy can swallow.
            val consent = pendingConsentIntent
            if (consent != null) {
                val pi = PendingIntent.getActivity(
                    this, 100,
                    Intent(consent).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M)
                        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
                    else PendingIntent.FLAG_UPDATE_CURRENT
                )
                builder.setContentIntent(pi)
            } else {
                val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
                if (launchIntent != null) {
                    val pendingIntent = PendingIntent.getActivity(
                        this, 0, launchIntent,
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) PendingIntent.FLAG_IMMUTABLE else 0
                    )
                    builder.setContentIntent(pendingIntent)
                }
            }
        } catch (t: Throwable) {
            Log.w(TAG, "Notification contentIntent unavailable: ${t.message}")
        }

        return builder.build()
    }
}
