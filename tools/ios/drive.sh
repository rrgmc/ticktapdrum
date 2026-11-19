#!/usr/bin/env bash
# Drives the iOS app on a simulator through a UI test, and saves its screenshots.
#
# Usage: tools/ios/drive.sh [--sim NAME | --device NAME] [--out DIR] 'step; step; ...'
#
# Steps: launch, activate, terminate, open BUNDLE_ID (another app, such as com.apple.DocumentsApp
# for Files), tap X Y, drag X1 Y1 X2 Y2, wait SECONDS, shot NAME,
# rotate portrait|landscapeLeft|landscapeRight|portraitUpsideDown, home, type TEXT, note TEXT.
# X and Y run from 0 to 1 across the app's window. Each `shot NAME` is saved as DIR/NAME.png.
# The simulator defaults to $DM_IOS_SIM, or "iPhone 17"; DIR defaults to ports/ios/build/shots.
# --device NAME drives a connected iPhone or iPad instead, signed with $DM_IOS_TEAM.
# Run tools/ios/build.sh first: this script uses the Rust library it builds.
set -euo pipefail

# shellcheck source=env.sh
source "$(dirname "${BASH_SOURCE[0]}")/env.sh"

usage() { sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

sim="${DM_IOS_SIM:-iPhone 17}"
device=""
out="$ios_build/shots"
while [ $# -gt 0 ]; do
    case "$1" in
        --sim) sim=${2:?--sim needs a simulator name}; shift 2 ;;
        --device) device=${2:?--device needs a device name}; shift 2 ;;
        --out) out=${2:?--out needs a folder}; shift 2 ;;
        -h | --help) usage; exit 0 ;;
        -*) die "unknown option $1" ;;
        *) break ;;
    esac
done
[ $# -eq 1 ] || { usage >&2; exit 2; }
steps=$1

[ -d "$ios_build/TickTapDrum.xcframework" ] || die "the Rust library is missing. Run tools/ios/build.sh first"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

# Regenerate the project, so it has the test target.
cd "$ios_project"
xcodegen generate --quiet

stamp="$(date +%Y%m%d-%H%M%S)"
result="$ios_build/results/drive-$stamp.xcresult"
log="$ios_build/xcodebuild-drive.log"
mkdir -p "$ios_build/results"

if [ -n "$device" ]; then
    [ -n "${DM_IOS_TEAM:-}" ] || die "a device needs signing: set DM_IOS_TEAM"
    target=(-destination "platform=iOS,name=$device" -allowProvisioningUpdates)
    echo "Driving the app on $device. The log is $log"
else
    target=(-destination "platform=iOS Simulator,name=$sim")
    echo "Driving the app on $sim. The log is $log"
fi
status=0
TEST_RUNNER_DM_STEPS="$steps" xcodebuild test \
    -project TickTapDrum.xcodeproj -scheme TickTapDrum "${target[@]}" \
    -derivedDataPath "$ios_build/derived/uitest" \
    -resultBundlePath "$result" >"$log" 2>&1 || status=$?

# Save each screenshot as NAME.png. manifest.json maps the exported files to their names.
if [ -d "$result" ]; then
    export_dir="$ios_build/results/drive-$stamp-attachments"
    xcrun xcresulttool export attachments --path "$result" --output-path "$export_dir" >/dev/null
    python3 -I - "$export_dir" "$out" <<'EOF'
import json, os, shutil, sys
src, dst = sys.argv[1], sys.argv[2]
with open(os.path.join(src, "manifest.json")) as f:
    manifest = json.load(f)
for test in manifest:
    for a in test["attachments"]:
        file = a["exportedFileName"]
        if not file.endswith(".png"):
            continue
        # Xcode adds "_<index>_<uuid>" to the name the test gave.
        name = a["suggestedHumanReadableName"]
        name = os.path.splitext(name)[0]
        parts = name.rsplit("_", 2)
        if len(parts) == 3 and len(parts[2]) == 36:
            name = parts[0]
        path = os.path.join(dst, name + ".png")
        shutil.copyfile(os.path.join(src, file), path)
        print(path)
EOF
fi

if [ "$status" -ne 0 ]; then
    grep -a -E 'error:|failed|XCTAssert|XCTFail|\*\* TEST FAILED|\*\* BUILD FAILED' "$log" >&2 || tail -20 "$log" >&2
    die "the drive failed; the full log is $log and the result bundle is $result"
fi
echo "Result bundle: $result"
