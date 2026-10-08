#!/usr/bin/env bash
# The platform probe on the emulator the workflow started (spec 044, T007, research R4).
# Usage: scripts/ci/android-platform-probe.sh <apk built with the feature platform-probe>
# One script, because the emulator action runs every line of its `script:` in a shell of its own.
set -euo pipefail
apk="$1"
log="$RUNNER_TEMP/platform-probe-logcat.txt"

echo "web view: $(adb shell dumpsys package com.google.android.webview \
  | sed -n 's/.*versionName=\([^ ]*\).*/\1/p' | head -1)"

adb install -r "$apk"
# Read by `platform_probe::requested` through `getprop`; the shell user may set `debug.*`.
adb shell setprop debug.holzi.probe 1
adb logcat -c
adb shell am start -n com.haex.holzi/.MainActivity

# The probe gives up after 90 s itself; wait a little longer for its line.
for _ in $(seq 1 50); do
  adb logcat -d > "$log"
  if grep -q HOLZI_PROBE_RESULT "$log"; then
    break
  fi
  sleep 3
done
adb shell setprop debug.holzi.probe 0

grep HOLZI_PROBE_RESULT "$log" || tail -n 200 "$log"
node scripts/run-platform-probe.ts --judge "$log"
