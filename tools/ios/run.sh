#!/usr/bin/env bash
# Installs the app built by tools/ios/build.sh and starts it, on a simulator or on a device.
#
# Usage: tools/ios/run.sh [--device] [--release]
#
# Simulator (the default): DM_IOS_SIM names the simulator, "iPhone 17" unless set; `xcrun simctl
# list devices available` lists them. The app's output stays on screen until Ctrl+C, which stops
# the app too.
# Device (--device): needs an app signed with DM_IOS_TEAM. DM_IOS_DEVICE names the device, or the
# first connected one is used; `xcrun devicectl list devices` lists them.
set -euo pipefail

# shellcheck source=env.sh
source "$(dirname "${BASH_SOURCE[0]}")/env.sh"

device=false
config=Debug
for arg in "$@"; do
    case "$arg" in
        --device) device=true ;;
        --release) config=Release ;;
        -h | --help)
            sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) die "unknown option $arg. Usage: tools/ios/run.sh [--device] [--release]" ;;
    esac
done

bundle_id=io.github.rrgmc.ticktapdrum

if $device; then
    [ -n "${DM_IOS_TEAM:-}" ] || die "a device needs a signed app: set DM_IOS_TEAM and run tools/ios/build.sh"
    app="$device_app_dir/Build/Products/$config-iphoneos/TickTapDrum.app"
    [ -d "$app" ] || die "no device app at $app. Run tools/ios/build.sh first"
    target="${DM_IOS_DEVICE:-}"
    if [ -z "$target" ]; then
        # The list also holds simulators and devices out of reach: take the first physical
        # device that is connected. plutil reads JSON and ships with macOS.
        json="$(mktemp)"
        xcrun devicectl list devices --json-output "$json" >/dev/null
        field() { plutil -extract "result.devices.$1.$2" raw -o - "$json" 2>/dev/null || true; }
        count="$(plutil -extract result.devices raw -o - "$json")"
        for ((i = 0; i < count; i++)); do
            if [ "$(field "$i" hardwareProperties.reality)" = physical ] &&
                [ "$(field "$i" connectionProperties.tunnelState)" = connected ]; then
                target="$(field "$i" identifier)"
                echo "Device: $(field "$i" deviceProperties.name)"
                break
            fi
        done
        rm -f "$json"
        [ -n "$target" ] || die "no connected device found. Connect and unlock one, or set DM_IOS_DEVICE"
    fi
    xcrun devicectl device install app --device "$target" "$app"
    xcrun devicectl device process launch --device "$target" --console "$bundle_id"
    exit 0
fi

app="$sim_app_dir/Build/Products/$config-iphonesimulator/TickTapDrum.app"
[ -d "$app" ] || die "no simulator app at $app. Run tools/ios/build.sh first"
sim="${DM_IOS_SIM:-iPhone 17}"
# Booting a booted simulator is an error worth ignoring.
xcrun simctl boot "$sim" 2>/dev/null || true
# Shows the simulator's screen, which is optional: the simulator also runs without a window.
# Older Xcode versions ship Simulator.app; newer ones show simulators in DeviceHub.
open -b com.apple.iphonesimulator 2>/dev/null || open -b com.apple.dt.Devices 2>/dev/null ||
    echo "note: no simulator window app found; the simulator runs without a window"
xcrun simctl install "$sim" "$app"
echo "Starting on $sim. Ctrl+C stops the app."
xcrun simctl launch --console-pty --terminate-running-process "$sim" "$bundle_id"
