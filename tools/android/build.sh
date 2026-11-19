#!/usr/bin/env bash
# Builds the Android APK: the Rust library with cargo-ndk, then the package with Gradle.
#
# Usage: tools/android/build.sh [--release | --apk-release]
#
#   (none)         debug library, debug APK: ports/android/app/build/outputs/apk/debug/app-debug.apk
#   --release      optimised library, debug APK as above. tools/android/run.sh installs that APK.
#   --apk-release  optimised library, release APK:
#                  ports/android/app/build/outputs/apk/release/app-release.apk
#
# The release APK is signed with the key named by DM_KEYSTORE_FILE, DM_KEYSTORE_PASSWORD,
# DM_KEY_ALIAS and DM_KEY_PASSWORD, or by ports/android/keystore.properties, or else with the
# debug key. DM_VERSION_NAME and DM_VERSION_CODE set its version (default 0.1.0 and 1).
set -euo pipefail

# shellcheck source=env.sh
source "$(dirname "${BASH_SOURCE[0]}")/env.sh"

usage="Usage: tools/android/build.sh [--release | --apk-release]"
profile=()
gradle_task=assembleDebug
out_apk="$apk"
case "${1:-}" in
    "") ;;
    --release) profile=(--release) ;;
    --apk-release)
        profile=(--release)
        gradle_task=assembleRelease
        out_apk="$release_apk"
        ;;
    -h | --help)
        sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *) die "unknown option $1. $usage" ;;
esac
[ $# -le 1 ] || die "too many options. $usage"

command -v cargo-ndk >/dev/null 2>&1 || die "cargo-ndk is missing. Install it with: cargo install cargo-ndk"

echo "SDK: $ANDROID_HOME"
echo "NDK: $ANDROID_NDK_HOME"

jni_libs="$android_project/app/src/main/jniLibs"
cd "$repo_root"
# --platform 26 matches minSdk in ports/android/app/build.gradle.
cargo ndk \
    --platform 26 \
    -t arm64-v8a -t x86_64 \
    -o "$(to_native "$jni_libs")" \
    build -p dm-android "${profile[@]}"

cd "$android_project"
# gradlew is a bash script. A plain `sh` is dash on Ubuntu, which cannot run it.
bash ./gradlew --console=plain "$gradle_task"

echo "APK: $out_apk"
