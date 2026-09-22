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
        private const val CONSENT_REQUEST_CODE = 0x5650

        @Volatile
        var watchdogStarted = false

        // Consent state, owned by the watchdog (v2rayNG ordering: the dialog
        // is launched from the foreground Activity BEFORE the service starts,
        // via startActivityForResult — the only consent path that has ever
        // worked on stock Android and MIUI in the field; every service-side
        // launch attempt (v0.2.84-89) was silently swallowed by MIUI).
        @Volatile
        var consentAttempts = 0

        @Volatile
        var awaitingConsentResume = false

        @Volatile
        var consentDialogReturned = false
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
            for (stale in listOf("vpn_pending", "vpn_stop", "tun_fd", "vpn_status", "vpn_consent")) {
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
            // nothing consumes Rust's vpn_pending signal, connect hangs for the full timeout
            // with zero UI feedback. The watchdog instead starts the service
            // lazily, exactly when the user taps connect (app is foreground,
            // so startForegroundService is always allowed).
            startVpnWatchdog(ctx.applicationContext)

            // Track the visible Activity process-wide, and fast-path the
            // consent round trip: when the user returns from the system
            // VPN-consent dialog, immediately re-signal vpn_pending so the
            // watchdog builds the tunnel within 400ms instead of waiting for
            // Rust's next 4s re-signal.
            (ctx.applicationContext as? Application)?.registerActivityLifecycleCallbacks(
                object : Application.ActivityLifecycleCallbacks {
                    override fun onActivityResumed(activity: Activity) {
                        NarcissusVpnService.currentActivity = activity
                        NarcissusVpnService.notifyActivityResumed()
                        requestNotificationPermissionOnce(activity)
                        if (awaitingConsentResume) {
                            awaitingConsentResume = false
                            consentDialogReturned = true
                            try {
                                File(ctx.dataDir, "vpn_pending").writeText("again")
                            } catch (_: Exception) {}
                        }
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

            // Consent ownership lives entirely in the watchdog below now:
            // exactly the v2rayNG sequence — foreground Activity launches the
            // SETTINGS-raised VpnConfirm via startActivityForResult, and the
            // VpnService is only started once prepare() returns null. The
            // service never launches the dialog itself; its sole waiting path
            // is the tappable consent notification + Settings guidance.
        } catch (e: Exception) {
            Log.e(TAG, "Failed to init provider", e)
        }
        return true
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
                        val content = try { pending.readText() } catch (_: Exception) { "new" }
                        try { pending.delete() } catch (_: Exception) {}
                        // "new" = a fresh user tap on 连接 (Rust's first signal);
                        // "again" = the 4s self-heal re-signal / consent return.
                        // A fresh tap re-arms the dialog allowance.
                        if (content != "again") {
                            consentAttempts = 0
                            consentDialogReturned = false
                            awaitingConsentResume = false
                        }
                        val prep = try {
                            android.net.VpnService.prepare(appCtx)
                        } catch (e: Exception) {
                            Log.w(TAG, "prepare() failed, letting the service re-check: ${e.message}")
                            null
                        }
                        if (prep == null) {
                            // Consented (or re-check deferred to the service):
                            // the only path that starts the tunnel service.
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
                        } else {
                            val act = NarcissusVpnService.currentActivity
                            when {
                                act == null || act.isFinishing -> {
                                    // Either our consent dialog is on screen
                                    // (Activity stopped) or the app is
                                    // backgrounded: do NOT start the service
                                    // without consent; the next re-signal
                                    // re-evaluates within ~4s.
                                    Log.i(TAG, "Consent missing, no foreground Activity; waiting")
                                }
                                !consentDialogReturned && consentAttempts < 2 -> {
                                    // EXACT v2rayNG pattern: foreground
                                    // Activity, launch-for-result, before any
                                    // service exists. One silent-drop retry
                                    // covers MIUI swallowing the first launch.
                                    consentAttempts++
                                    awaitingConsentResume = true
                                    try { status.writeText("consent_required") } catch (_: Exception) {}
                                    act.runOnUiThread {
                                        try {
                                            act.startActivityForResult(prep, CONSENT_REQUEST_CODE)
                                            Log.i(TAG, "Consent dialog launched via startActivityForResult (attempt $consentAttempts)")
                                        } catch (e: Exception) {
                                            Log.w(TAG, "Consent launch threw: ${e.message}")
                                            awaitingConsentResume = false
                                            consentDialogReturned = true
                                            try {
                                                status.writeText("consent_activity_failed:${e.javaClass.simpleName}")
                                            } catch (_: Exception) {}
                                            // Hand over to the service's
                                            // waiting path: consent
                                            // notification + Settings guidance.
                                            try {
                                                NarcissusVpnService.startVpn(appCtx)
                                            } catch (_: Exception) {}
                                        }
                                    }
                                }
                                else -> {
                                    // Dialog was shown and dismissed without
                                    // granting, or both attempts got silently
                                    // dropped: start the service so it posts
                                    // the tappable consent notification and
                                    // the Settings-guidance stage while Rust
                                    // keeps re-signalling for auto-continue.
                                    try {
                                        NarcissusVpnService.startVpn(appCtx)
                                    } catch (e: Exception) {
                                        Log.e(TAG, "startVpn (waiting) failed", e)
                                    }
                                }
                            }
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
