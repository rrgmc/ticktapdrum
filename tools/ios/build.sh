#!/usr/bin/env bash
# Builds the iOS app: the Rust library for devices and the simulator, packed as an xcframework,
# then the Xcode project from ports/ios/project.yml, then the app itself.
#
# Usage: tools/ios/build.sh [--release]
#
# The simulator app is always built. The device app is signed when DM_IOS_TEAM holds your Apple
# development team ID; without it the device app is built unsigned, which still checks that it
# compiles and links. The library is a debug build unless --release is given.
set -euo pipefail

# shellcheck source=env.sh
source "$(dirname "${BASH_SOURCE[0]}")/env.sh"

profile=()
config=Debug
case "${1:-}" in
    "") ;;
    --release)
        profile=(--release)
        config=Release
        ;;
    -h | --help)
        sed -n '2,9p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *) die "unknown option $1. Usage: tools/ios/build.sh [--release]" ;;
esac

command -v xcodegen >/dev/null 2>&1 || die "XcodeGen is missing. Install it with: brew install xcodegen"
xcodebuild -showsdks 2>/dev/null | grep -q -- '-sdk iphoneos' ||
    die "Xcode has no iOS platform. Install it with: xcodebuild -downloadPlatform iOS"

cd "$repo_root"
lib_profile=debug
[ "$config" = Release ] && lib_profile=release
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    cargo build -p dm-ios --lib --target "$target" "${profile[@]}"
done

# Two arm64 slices cannot share one archive, so they go in an xcframework.
# -create-xcframework refuses to overwrite its output.
rm -rf "$ios_build/TickTapDrum.xcframework"
mkdir -p "$ios_build"
xcodebuild -create-xcframework \
    -library "$target_dir/aarch64-apple-ios/$lib_profile/libticktapdrum.a" \
    -library "$target_dir/aarch64-apple-ios-sim/$lib_profile/libticktapdrum.a" \
    -output "$ios_build/TickTapDrum.xcframework" >/dev/null

cd "$ios_project"
xcodegen generate --quiet

# Builds the app, keeping xcodebuild's long output in a log and showing only its errors.
build_app() {
    local name=$1
    shift
    local log="$ios_build/xcodebuild-$name.log"
    if ! xcodebuild -project TickTapDrum.xcodeproj -scheme TickTapDrum -configuration "$config" \
        "$@" build >"$log" 2>&1; then
        # -a: the log holds some binary bytes, which would make grep print only "Binary file".
        grep -a -E 'error:|\*\* BUILD FAILED' "$log" >&2 || tail -20 "$log" >&2
        die "the $name app did not build; the full log is $log"
    fi
}

build_app simulator -destination 'generic/platform=iOS Simulator' -derivedDataPath "$sim_app_dir"
echo "Simulator app: $sim_app_dir/Build/Products/$config-iphonesimulator/TickTapDrum.app"

if [ -n "${DM_IOS_TEAM:-}" ]; then
    build_app device -destination 'generic/platform=iOS' -derivedDataPath "$device_app_dir" \
        -allowProvisioningUpdates DM_IOS_TEAM="$DM_IOS_TEAM"
    echo "Device app: $device_app_dir/Build/Products/$config-iphoneos/TickTapDrum.app"
else
    build_app device -destination 'generic/platform=iOS' -derivedDataPath "$device_app_dir" \
        CODE_SIGNING_ALLOWED=NO
    echo "Device app built unsigned, as a compile check. Set DM_IOS_TEAM to sign it."
fi
