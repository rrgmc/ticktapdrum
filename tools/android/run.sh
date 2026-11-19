#!/usr/bin/env bash
# Installs the APK that build.sh made, starts the app and shows its log until Ctrl+C.
#
# Usage: tools/android/run.sh
#
# With more than one device or emulator attached, set ANDROID_SERIAL to choose one
# (see "adb devices").
set -euo pipefail

# shellcheck source=env.sh
source "$(dirname "${BASH_SOURCE[0]}")/env.sh"

[ -n "$ADB" ] || die "cannot find adb. Install the SDK platform-tools."
[ -f "$apk" ] || die "no APK at $apk. Run tools/android/build.sh first."

"$ADB" wait-for-device
"$ADB" install -r "$(to_native "$apk")"
"$ADB" shell am force-stop "$app_id"
"$ADB" logcat -c
# Started the way the home screen starts it.
"$ADB" shell am start -W \
    -a android.intent.action.MAIN -c android.intent.category.LAUNCHER \
    -n "$app_id/.MainActivity"

pid=""
for _ in 1 2 3 4 5 6 7 8 9 10; do
    pid="$("$ADB" shell pidof "$app_id" | tr -d '\r' || true)"
    [ -n "$pid" ] && break
    sleep 1
done
[ -n "$pid" ] || die "the app did not start. See: adb logcat -d"

echo "Showing the log of process $pid. Press Ctrl+C to stop."
exec "$ADB" logcat --pid="$pid" -v brief
