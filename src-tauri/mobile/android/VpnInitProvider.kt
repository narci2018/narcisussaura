package com.narcissus.aura

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.provider.Settings
import android.util.Log
import java.io.File

/**
 * ContentProvider that auto-initializes before any Activity.
 * Starts NarcissusVpnService in standby mode and writes a stable
 * device ID (ANDROID_ID) so the Rust backend can use it for auth.
 */
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
                Log.i(TAG, "Wrote stable ANDROID_ID as machine_id: $androidId")
            }
            Log.i(TAG, "Starting NarcissusVpnService in standby mode")
            NarcissusVpnService.startStandby(ctx)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to auto-start VpnService", e)
        }
        return true
    }

    override fun query(uri: Uri, p: Array<out String>?, s: String?, a: Array<out String>?, sort: String?): Cursor? = null
    override fun getType(uri: Uri): String? = null
    override fun insert(uri: Uri, v: ContentValues?): Uri? = null
    override fun delete(uri: Uri, s: String?, a: Array<out String>?): Int = 0
    override fun update(uri: Uri, v: ContentValues?, s: String?, a: Array<out String>?): Int = 0
}
