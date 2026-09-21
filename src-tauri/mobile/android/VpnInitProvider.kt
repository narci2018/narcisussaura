package com.narcissus.aura

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.provider.Settings
import android.util.Log
import java.io.File

class VpnInitProvider : ContentProvider() {

    companion object {
        private const val TAG = "VpnInitProvider"
    }

    override fun onCreate(): Boolean {
        val ctx = context ?: return true
        try {
            val androidId = Settings.Secure.getString(ctx.contentResolver, Settings.Secure.ANDROID_ID)
            if (androidId != null) {
                val idFile = File(ctx.filesDir, "machine_id")
                idFile.writeText(androidId)
                Log.i(TAG, "Wrote stable ANDROID_ID as machine_id: $androidId to ${idFile.absolutePath}")
            }

            extractBinaries(ctx)

            Log.i(TAG, "Starting NarcissusVpnService in standby mode")
            NarcissusVpnService.startStandby(ctx)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to auto-start VpnService", e)
        }
        return true
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
