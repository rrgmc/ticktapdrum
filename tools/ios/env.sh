# Shared setup for the iOS scripts. Source it; do not run it.

die() {
    echo "error: $*" >&2
    exit 1
}

[ "$(uname -s)" = Darwin ] || die "the iOS app builds on macOS only"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ios_project="$repo_root/ports/ios"
ios_build="$ios_project/build"

# xcodebuild needs the full Xcode. When the active developer folder is the command line tools,
# use Xcode from its usual place instead of asking for `sudo xcode-select`.
if ! xcodebuild -version >/dev/null 2>&1; then
    if [ -z "${DEVELOPER_DIR:-}" ] && [ -d /Applications/Xcode.app/Contents/Developer ]; then
        export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
    fi
    xcodebuild -version >/dev/null 2>&1 ||
        die "xcodebuild needs the full Xcode. Install it from the App Store, or set DEVELOPER_DIR to its Contents/Developer folder"
fi
developer_dir="${DEVELOPER_DIR:-$(xcode-select -p)}"

# Must match deploymentTarget in ports/ios/project.yml, so code the `cc` crate compiles links.
export IPHONEOS_DEPLOYMENT_TARGET=15.0

# The app's version, which ports/ios/project.yml reads. XcodeGen leaves an unset variable as it
# is, so both get a value here. tools/ios/package.sh sets them for a release.
export DM_VERSION_NAME="${DM_VERSION_NAME:-0.1.0}"
export DM_VERSION_CODE="${DM_VERSION_CODE:-1}"

# Cargo's output folder, which is not always ./target.
target_dir="$(cd "$repo_root" && cargo metadata --format-version 1 --no-deps |
    sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
[ -n "$target_dir" ] || die "could not find cargo's target folder"

sim_app_dir="$ios_build/derived/sim"
device_app_dir="$ios_build/derived/device"
