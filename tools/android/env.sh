# Finds the Android SDK and NDK. Sourced by build.sh and run.sh; not meant to be run on its own.
#
# Sets and exports ANDROID_HOME and ANDROID_NDK_HOME, and sets ADB. The environment wins: an
# ANDROID_HOME, ANDROID_SDK_ROOT or ANDROID_NDK_HOME that is already set is used as it is.
# Otherwise the SDK is looked for where Android Studio installs it, and the NDK is the newest
# version inside the SDK's ndk folder.

# Paths handed to Windows programs (cargo, Gradle) must be Windows paths. Paths this script tests
# must be POSIX paths in Git Bash. These two helpers convert where cygpath exists and do nothing
# elsewhere.
to_posix() { if command -v cygpath >/dev/null 2>&1; then cygpath -u "$1"; else printf '%s\n' "$1"; fi; }
to_native() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s\n' "$1"; fi; }

die() { echo "error: $*" >&2; exit 1; }

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
android_project="$repo_root/ports/android"

sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$sdk" ]; then
    case "$(uname -s)" in
        MINGW* | MSYS* | CYGWIN*) [ -n "${LOCALAPPDATA:-}" ] && sdk="$LOCALAPPDATA/Android/Sdk" ;;
        Darwin) sdk="$HOME/Library/Android/sdk" ;;
        *) sdk="$HOME/Android/Sdk" ;;
    esac
fi
[ -n "$sdk" ] || die "cannot find the Android SDK. Set ANDROID_HOME."
sdk="$(to_posix "$sdk")"
[ -d "$sdk" ] || die "no Android SDK at $sdk. Set ANDROID_HOME."

ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [ -z "$ndk" ]; then
    newest="$(ls -1 "$sdk/ndk" 2>/dev/null | sort -V | tail -n 1)"
    [ -n "$newest" ] && ndk="$sdk/ndk/$newest"
fi
[ -n "$ndk" ] || die "cannot find the Android NDK. Install one with the SDK manager or set ANDROID_NDK_HOME."
ndk="$(to_posix "$ndk")"
[ -d "$ndk" ] || die "no Android NDK at $ndk. Set ANDROID_NDK_HOME."

ANDROID_HOME="$(to_native "$sdk")"
ANDROID_NDK_HOME="$(to_native "$ndk")"
export ANDROID_HOME ANDROID_NDK_HOME
# Gradle reads ANDROID_HOME. An older ANDROID_SDK_ROOT that disagrees with it makes Gradle fail.
unset ANDROID_SDK_ROOT

ADB="$sdk/platform-tools/adb"
[ -x "$ADB" ] || [ -x "$ADB.exe" ] || ADB="$(command -v adb || true)"

apk="$android_project/app/build/outputs/apk/debug/app-debug.apk"
release_apk="$android_project/app/build/outputs/apk/release/app-release.apk"
app_id="dev.rrgmc.ticktapdrum"
