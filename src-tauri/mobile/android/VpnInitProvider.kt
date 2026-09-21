package com.narcissus.aura

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.util.Log

/**
 * ContentProvider that auto-initializes before any Activity.
 * Starts NarcissusVpnService in standby mode so it's ready
 * when the Rust backend signals via vpn_pending file.
 */
class VpnInitProvider : ContentProvider() {

    companion object {
        private const val TAG = "VpnInitProvider"
    }

    override fun onCreate(): Boolean {
        val ctx = context ?: return true
        try {
            Log.i(TAG, "Auto-starting NarcissusVpnService in standby mode")
            NarcissusVpnService.startVpn(ctx)
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
