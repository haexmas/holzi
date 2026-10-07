package space.haex.holzi.android

import android.app.Activity
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Plugin

/** holzi's Android platform code (spec 043, ADR-0010); the core calls it directly. */
@TauriPlugin
class HolziAndroidPlugin(private val activity: Activity) : Plugin(activity)
