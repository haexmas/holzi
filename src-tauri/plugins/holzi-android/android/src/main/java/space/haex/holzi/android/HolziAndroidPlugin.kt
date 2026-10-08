package space.haex.holzi.android

import android.app.Activity
import android.net.Uri
import android.provider.OpenableColumns
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class UriArgs {
    lateinit var uri: String
}

/** holzi's Android platform code (spec 043, ADR-0010); the core calls it directly. */
@TauriPlugin
class HolziAndroidPlugin(private val activity: Activity) : Plugin(activity) {
    /** The name a document provider shows for a chosen file (contract picked-file.md). */
    @Command
    fun displayName(invoke: Invoke) {
        val args = invoke.parseArgs(UriArgs::class.java)
        val result = JSObject()
        try {
            activity.contentResolver
                .query(Uri.parse(args.uri), arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { cursor ->
                    if (cursor.moveToFirst() && !cursor.isNull(0)) {
                        result.put("name", cursor.getString(0))
                    }
                }
        } catch (e: Exception) {
            // No name then; the core falls back to the last part of the address.
        }
        invoke.resolve(result)
    }
}
