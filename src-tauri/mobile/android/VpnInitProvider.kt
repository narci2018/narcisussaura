package com.narcissus.aura

import android.app.Activity
import android.app.Application
import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import java.io.File

class VpnInitProvider : ContentProvider() {

    companion object {
        private const val TAG = "VpnInitProvider"

        @Volatile
        var watchdogStarted = false
    }

    override fun onCreate(): Boolean {
        val ctx = context ?: return true
        try {
            // Persist any uncaught JVM exception (main thread or our watchdog)
            // to crash_log so the Rust side can surface the real stack in the
            // UI on the next launch — our only crash channel without adb.
            val crashFile = File(ctx.dataDir, "crash_log")
            val prevHandler = Thread.getDefaultUncaughtExceptionHandler()
            Thread.setDefaultUncaughtExceptionHandler { t, e ->
                try {
                    val sw = java.io.StringWriter()
                    e.printStackTrace(java.io.PrintWriter(sw))
                    crashFile.writeText("线程 ${t.name}:\n$sw")
                } catch (_: Exception) {}
                prevHandler?.uncaughtException(t, e)
            }

            // Drop tunnel handshake files left behind by a previous (possibly
            // crashed) session: a stale vpn_pending would otherwise make the
            // watchdog start the VPN service and pop the consent dialog at
            // every app launch, and a stale vpn_status poisons diagnostics.
            for (stale in listOf("vpn_pending", "vpn_stop", "tun_fd", "vpn_status")) {
                try { File(ctx.dataDir, stale).delete() } catch (_: Exception) {}
            }

            // Expose nativeLibraryDir (the only reliably executable dir; dataDir is
            // noexec for targetSdk>=29) so the Rust side can locate lib<core>.so.
            File(ctx.dataDir, "native_lib_dir").writeText(ctx.applicationInfo.nativeLibraryDir)

            val idFile = File(ctx.dataDir, "machine_id")
            // Use stable UUID: if file exists, keep it; otherwise generate new one
            if (!idFile.exists() || idFile.readText().trim().isEmpty()) {
                val stableId = java.util.UUID.nameUUIDFromBytes(
                    (Settings.Secure.getString(ctx.contentResolver, Settings.Secure.ANDROID_ID)
                        ?: "unknown").toByteArray()
                ).toString().replace("-", "").take(16)
                idFile.writeText(stableId)
                Log.i(TAG, "Generated stable machine_id: $stableId -> ${idFile.absolutePath}")
            } else {
                Log.i(TAG, "machine_id already exists: ${idFile.readText().trim()}")
            }

            extractBinaries(ctx)

            // The tunnel-request watcher MUST live in the app process, not in
            // the VpnService: Android 12+ foreground-service-start restrictions
            // (and aggressive OEM/MIUI killing) can prevent the service from
            // ever starting, and a service-side watcher would die with it. If
            // nothing consumes Rust's vpn_pending signal, connect hangs for 120s
            // with zero UI feedback. The watchdog instead starts the service
            // lazily, exactly when the user taps connect (app is foreground,
            // so startForegroundService is always allowed).
            startVpnWatchdog(ctx.applicationContext)

            // Track the visible Activity process-wide. The provider runs
            // before the first onActivityResumed, so currentActivity is
            // populated even though the VpnService only starts later, when
            // the user taps connect. The consent dialog must launch from a
            // resumed Activity (BAL restrictions), and its dismissal resumes
            // the Activity, which re-triggers tunnel establishment.
            (ctx.applicationContext as? Application)?.registerActivityLifecycleCallbacks(
                object : Application.ActivityLifecycleCallbacks {
                    override fun onActivityResumed(activity: Activity) {
                        NarcissusVpnService.currentActivity = activity
                        NarcissusVpnService.notifyActivityResumed()
                        requestNotificationPermissionOnce(activity)
                    }
                    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {}
                    override fun onActivityStarted(activity: Activity) {}
                    override fun onActivityPaused(activity: Activity) {}
                    override fun onActivityStopped(activity: Activity) {
                        if (NarcissusVpnService.currentActivity === activity) {
                            NarcissusVpnService.currentActivity = null
                        }
                    }
                    override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}
                    override fun onActivityDestroyed(activity: Activity) {
                        if (NarcissusVpnService.currentActivity === activity) {
                            NarcissusVpnService.currentActivity = null
                        }
                    }
                }
            )

            // MIUI silently DROPS activities started by a handler post that is
            // not tied to a user tap ("后台弹出界面" policy) — no exception, no
            // dialog. That is why the VPN-consent dialog never rendered even
            // though startActivity "succeeded". A native in-window button is
            // the only bulletproof path: its click handler is user-input
            // bound, so the consent dialog launch can never be intercepted.
            startConsentOverlayPoller(ctx.applicationContext)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to init provider", e)
        }
        return true
    }

    private val mainHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private var consentOverlay: android.widget.Button? = null
    private var overlayHost: Activity? = null

    private fun startConsentOverlayPoller(appCtx: android.content.Context) {
        mainHandler.post(object : Runnable {
            override fun run() {
                try {
                    refreshConsentOverlay(appCtx)
                } catch (e: Exception) {
                    Log.w(TAG, "consent overlay refresh failed: ${e.message}")
                }
                mainHandler.postDelayed(this, 600)
            }
        })
    }

    private fun refreshConsentOverlay(appCtx: android.content.Context) {
        val status = try {
            File(appCtx.dataDir, "vpn_status").readText().trim()
        } catch (_: Exception) {
            ""
        }
        val act = NarcissusVpnService.currentActivity
        val needsConsent = act != null &&
            NarcissusVpnService.tunnelPending() &&
            status.startsWith("consent")
        if (!needsConsent) {
            removeConsentOverlay()
            return
        }
        if (consentOverlay != null && overlayHost === act) return

        val activity = act ?: return
        val btn = android.widget.Button(activity).apply {
            text = "⚠ 需要 VPN 授权：点击此处完成"
            setBackgroundColor(0xFFF59E0B.toInt())
            setTextColor(0xFF111827.toInt())
            setOnClickListener {
                val a = NarcissusVpnService.currentActivity ?: return@setOnClickListener
                try {
                    val prep = android.net.VpnService.prepare(a)
                    if (prep != null) {
                        a.startActivity(prep)
                    } else {
                        // Consent already granted (e.g. "always allow"): just
                        // nudge the service to build the tunnel now.
                        NarcissusVpnService.notifyActivityResumed()
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "overlay consent launch failed", e)
                    try {
                        File(appCtx.dataDir, "vpn_status")
                            .writeText("consent_overlay_failed:${e.javaClass.simpleName}")
                    } catch (_: Exception) {}
                }
            }
        }
        val lp = android.widget.FrameLayout.LayoutParams(
            android.widget.FrameLayout.LayoutParams.MATCH_PARENT,
            android.widget.FrameLayout.LayoutParams.WRAP_CONTENT
        )
        lp.setMargins(48, 220, 48, 0)
        try {
            (activity.window.decorView as? android.view.ViewGroup)?.addView(btn, lp)
            consentOverlay = btn
            overlayHost = activity
            Log.i(TAG, "Consent overlay button attached")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to attach consent overlay: ${e.message}")
        }
    }

    private fun removeConsentOverlay() {
        val btn = consentOverlay ?: run { overlayHost = null; return }
        try {
            (btn.parent as? android.view.ViewGroup)?.removeView(btn)
        } catch (_: Exception) {}
        consentOverlay = null
        overlayHost = null
    }

    @Volatile
    private var notifPermRequested = false

    /**
     * MIUI/ColorOS startForeground() throws when the notification channel is
     * blocked, which previously killed the app via
     * ForegroundServiceDidNotStartInTimeException. Ask for POST_NOTIFICATIONS
     * once, from the first resumed Activity, so the foreground notification
     * can be posted. The raw permission string avoids compile-SDK coupling.
     */
    private fun requestNotificationPermissionOnce(activity: Activity) {
        if (notifPermRequested) return
        if (android.os.Build.VERSION.SDK_INT < 33) return
        notifPermRequested = true
        try {
            activity.requestPermissions(arrayOf("android.permission.POST_NOTIFICATIONS"), 0)
        } catch (_: Exception) {}
    }

    private fun startVpnWatchdog(appCtx: android.content.Context) {
        if (watchdogStarted) return
        synchronized(this) {
            if (watchdogStarted) return
            watchdogStarted = true
        }
        val dataDir = appCtx.dataDir
        Thread({
            val pending = File(dataDir, "vpn_pending")
            val stop = File(dataDir, "vpn_stop")
            val status = File(dataDir, "vpn_status")
            while (true) {
                try {
                    // User hit terminate: tear the tunnel down. startService is
                    // safe because disconnect only happens while the UI is up.
                    if (stop.exists()) {
                        try { stop.delete() } catch (_: Exception) {}
                        try {
                            appCtx.startService(
                                android.content.Intent(appCtx, NarcissusVpnService::class.java)
                                    .setAction(NarcissusVpnService.ACTION_DISCONNECT)
                            )
                        } catch (e: Exception) {
                            Log.w(TAG, "stop dispatch failed (service likely dead): ${e.message}")
                        }
                    }
                    if (pending.exists()) {
                        try { pending.delete() } catch (_: Exception) {}
                        try {
                            NarcissusVpnService.startVpn(appCtx)
                        } catch (e: Exception) {
                            Log.e(TAG, "startVpn failed", e)
                            try {
                                status.writeText(
                                    "service_start_failed:${e.javaClass.simpleName}:${e.message}"
                                )
                            } catch (_: Exception) {}
                        }
                    }
                } catch (e: Exception) {
                    Log.w(TAG, "watchdog iteration failed", e)
                }
                try {
                    Thread.sleep(400)
                } catch (_: InterruptedException) {
                    return@Thread
                }
            }
        }, "vpn-watchdog").apply { isDaemon = true; start() }
    }

    private fun extractBinaries(ctx: android.content.Context) {
        val binariesDir = File(ctx.dataDir, "binaries")
        binariesDir.mkdirs()

        val assetManager = ctx.assets
        try {
            val rootAssets = assetManager.list("") ?: emptyArray()
            Log.i(TAG, "APK root assets: ${rootAssets.joinToString()}")

            if ("binaries" in rootAssets) {
                extractAssetDir(assetManager, "binaries", binariesDir)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to extract binaries from APK assets", e)
        }
    }

    private fun extractAssetDir(assetManager: android.content.res.AssetManager, assetPath: String, targetDir: File) {
        val entries = assetManager.list(assetPath) ?: return
        for (entry in entries) {
            val fullPath = "$assetPath/$entry"
            val targetFile = File(targetDir, entry)
            val subEntries = assetManager.list(fullPath)
            if (subEntries != null && subEntries.isNotEmpty()) {
                targetFile.mkdirs()
                extractAssetDir(assetManager, fullPath, targetFile)
            } else {
                if (targetFile.exists()) continue
                try {
                    assetManager.open(fullPath).use { input ->
                        targetFile.outputStream().use { output ->
                            input.copyTo(output)
                        }
                    }
                    targetFile.setExecutable(true, false)
                    Log.i(TAG, "Extracted: $fullPath -> ${targetFile.absolutePath} (${targetFile.length()} bytes)")
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to extract: $fullPath", e)
                }
            }
        }
    }

    override fun query(uri: Uri, p: Array<out String>?, s: String?, a: Array<out String>?, sort: String?): Cursor? = null
    override fun getType(uri: Uri): String? = null
    override fun insert(uri: Uri, v: ContentValues?): Uri? = null
    override fun delete(uri: Uri, s: String?, a: Array<out String>?): Int = 0
    override fun update(uri: Uri, v: ContentValues?, s: String?, a: Array<out String>?): Int = 0
}
