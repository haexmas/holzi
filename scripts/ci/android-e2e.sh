#!/usr/bin/env bash
# One shard of the e2e suite on the emulator the workflow started (spec 043, contracts/ci.md).
# Usage: scripts/ci/android-e2e.sh <shard> <shards> <apk>
# One script, because the emulator action runs every line of its `script:` in a shell of its own.
set -euo pipefail
shard="$1"
shards="$2"
apk="$3"

# chromedriver must have the major version of the emulator's web view (Chrome for Testing).
major="$(adb shell dumpsys package com.google.android.webview \
  | sed -n 's/.*versionName=\([0-9][0-9]*\)\..*/\1/p' | head -1)"
if [ -z "$major" ]; then
  echo "the emulator reports no web view version" >&2
  exit 1
fi
url="$(curl -fsSL https://googlechromelabs.github.io/chrome-for-testing/latest-versions-per-milestone-with-downloads.json \
  | node -e '
    let text = ""
    process.stdin.on("data", (chunk) => (text += chunk))
    process.stdin.on("end", () => {
      const milestone = JSON.parse(text).milestones[process.argv[1]]
      const download = milestone?.downloads?.chromedriver?.find((d) => d.platform === "linux64")
      if (!download) process.exit(1)
      console.log(download.url)
    })' "$major")"
curl -fsSL -o "$RUNNER_TEMP/chromedriver.zip" "$url"
unzip -q -o "$RUNNER_TEMP/chromedriver.zip" -d "$RUNNER_TEMP"
export E2E_CHROMEDRIVER="$RUNNER_TEMP/chromedriver-linux64/chromedriver"

corepack pnpm test:e2e --platform android --apk "$apk" --shard "$shard/$shards"
