package com.haex.holzi

import android.os.Bundle
import android.system.Os
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Spec 043: Rust and SQLite put temporary files under TMPDIR and fall back to
    // /data/local/tmp, which an app cannot write. Set before super.onCreate starts the Rust side,
    // while no other thread reads the environment.
    Os.setenv("TMPDIR", cacheDir.absolutePath, true)
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
