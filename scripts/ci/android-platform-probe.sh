#!/usr/bin/env bash
# The platform probe on the emulator the workflow started (spec 044, T007, research R4).
# Usage: scripts/ci/android-platform-probe.sh <apk built with the feature platform-probe>
# One script, because the emulator action runs every line of its `script:` in a shell of its own.
set -euo pipefail
apk="$1"
package=com.haex.holzi
result="$RUNNER_TEMP/platform-probe-result.txt"

echo "web view: $(adb shell dumpsys package com.google.android.webview \
  | sed -n 's/.*versionName=\([^ ]*\).*/\1/p' | head -1)"

adb install -r "$apk"
# Read by `platform_probe::requested` through `getprop`; the shell user may set `debug.*`.
adb shell setprop debug.holzi.probe 1
echo "debug.holzi.probe=$(adb shell getprop debug.holzi.probe)"
adb shell am start -n "$package/.MainActivity"

# The probe writes its line into holzi's cache directory (`platform_probe::RESULT_FILE`); a debug
# APK lets the shell read it with `run-as`. The probe gives up after 90 s itself.
: > "$result"
for _ in $(seq 1 50); do
  if adb shell run-as "$package" cat cache/platform-probe-result.txt > "$result" 2>/dev/null \
    && grep -q HOLZI_PROBE_RESULT "$result"; then
    break
  fi
  sleep 3
done
adb shell setprop debug.holzi.probe 0

if ! grep -q HOLZI_PROBE_RESULT "$result"; then
  echo "no result; started marker: $(adb shell run-as "$package" cat cache/platform-probe-started.txt 2>&1 || true)"
  echo "process: $(adb shell pidof "$package" || echo 'not running')"
  adb logcat -d -s holzi-probe || true
  adb logcat -d -b all | grep -iE "holzi|tauri|rust|chromium|AndroidRuntime" | tail -n 200 || true
fi
cat "$result"
node scripts/run-platform-probe.ts --judge "$result"
