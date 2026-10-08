package space.haex.holzi.android

import android.app.Activity
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.view.WindowManager
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class UriArgs {
    lateinit var uri: String
}

@InvokeArg
class SecureArgs {
    var enabled: Boolean = true
}

@InvokeArg
class InsetsArgs {
    lateinit var channel: Channel
}

/** holzi's Android platform code (spec 043, ADR-0010); the core calls it directly. */
@TauriPlugin
class HolziAndroidPlugin(private val activity: Activity) : Plugin(activity) {
    @Volatile private var insetsChannel: Channel? = null
    @Volatile private var lastInsets: JSObject? = null

    /**
     * The window is drawn edge to edge, so the page must keep its controls clear of the system bars,
     * the display cutout and the keyboard itself (FR-011, FR-012). The space they take goes to the
     * core in CSS pixels; the default handling of the web view still runs.
     */
    override fun load(webView: WebView) {
        super.load(webView)
        ViewCompat.setOnApplyWindowInsetsListener(webView) { view, insets ->
            val bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
            )
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            val density = view.resources.displayMetrics.density
            val next = JSObject()
            next.put("top", bars.top / density)
            next.put("right", bars.right / density)
            next.put("bottom", bars.bottom / density)
            next.put("left", bars.left / density)
            next.put("keyboard", maxOf(0, ime.bottom - bars.bottom) / density)
            lastInsets = next
            insetsChannel?.send(next)
            ViewCompat.onApplyWindowInsets(view, insets)
        }
        ViewCompat.requestApplyInsets(webView)
    }

    /** Sends the insets now and on every change to the core's channel. */
    @Command
    fun watchInsets(invoke: Invoke) {
        val args = invoke.parseArgs(InsetsArgs::class.java)
        insetsChannel = args.channel
        lastInsets?.let { args.channel.send(it) }
        invoke.resolve()
    }

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

    /**
     * Screen capture protection (FR-011a): no screenshots or recordings of the window, and an empty
     * preview in the recent apps view (API 33+), while a vault is open.
     */
    @Command
    fun setSecure(invoke: Invoke) {
        val args = invoke.parseArgs(SecureArgs::class.java)
        activity.runOnUiThread {
            if (args.enabled) {
                activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                activity.window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                activity.setRecentsScreenshotEnabled(!args.enabled)
            }
            invoke.resolve()
        }
    }
}
