#!/usr/bin/env bash
# Builds a release of the iOS app and packs it for download, in dist/ at the repository root.
#
# Usage: tools/ios/package.sh VERSION
#
# VERSION is the release version, such as 1.2.3 or 1.2.3-beta.1. It names the files and sets the
# app's version: DM_VERSION_NAME becomes 1.2.3 and DM_VERSION_CODE 10203, unless they are set.
# Writes ticktapdrum-VERSION-ios.ipa, the device app, and ticktapdrum-VERSION-ios-simulator.zip,
# the simulator app. The device app is signed only when DM_IOS_TEAM is set.
set -euo pipefail

# env.sh gives the version variables defaults, so note first which ones the caller set.
name_given="${DM_VERSION_NAME:-}"
code_given="${DM_VERSION_CODE:-}"

# shellcheck source=env.sh
source "$(dirname "${BASH_SOURCE[0]}")/env.sh"

usage="Usage: tools/ios/package.sh VERSION"
case "${1:-}" in
    "") die "no version given. $usage" ;;
    -h | --help)
        sed -n '2,9p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    -*) die "unknown option $1. $usage" ;;
esac
[ $# -eq 1 ] || die "too many arguments. $usage"
version=$1

# The app's version has up to three numbers and no pre-release suffix, so 1.2.3-beta.1 gives
# 1.2.3. A leading "v", as in a tag, is dropped too.
core="${version#v}"
core="${core%%[-+]*}"
[[ "$core" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] ||
    die "version $version does not start with numbers such as 1.2.3"
IFS=. read -r major minor patch <<<"$core"
# 10# reads 08 as eight, not as a bad octal number.
major=$((10#$major))
minor=$((10#${minor:-0}))
patch=$((10#${patch:-0}))
((minor < 100 && patch < 100)) ||
    die "version $version has a part over 99, which does not fit the build number"
export DM_VERSION_NAME="${name_given:-$major.$minor.$patch}"
export DM_VERSION_CODE="${code_given:-$((major * 10000 + minor * 100 + patch))}"
echo "Version $DM_VERSION_NAME, build $DM_VERSION_CODE"

"$(dirname "${BASH_SOURCE[0]}")/build.sh" --release

device_app="$device_app_dir/Build/Products/Release-iphoneos/TickTapDrum.app"
sim_app="$sim_app_dir/Build/Products/Release-iphonesimulator/TickTapDrum.app"
[ -d "$device_app" ] || die "no device app at $device_app"
[ -d "$sim_app" ] || die "no simulator app at $sim_app"

dist="$repo_root/dist"
mkdir -p "$dist"
ipa="$dist/ticktapdrum-$version-ios.ipa"
sim_zip="$dist/ticktapdrum-$version-ios-simulator.zip"
rm -f "$ipa" "$sim_zip"

# An IPA is a zip with the app in a Payload folder. Without DM_IOS_TEAM the app is unsigned, and
# iOS installs no unsigned app: a sideloading tool must sign it again first. With DM_IOS_TEAM it
# carries a development signature, which installs only on devices registered to that team.
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
mkdir "$staging/Payload"
ditto "$device_app" "$staging/Payload/TickTapDrum.app"
(cd "$staging" && ditto -c -k --keepParent Payload "$ipa")
echo "Device app: $ipa"

# ditto, unlike zip without -y, keeps the symlinks inside the app.
ditto -c -k --keepParent "$sim_app" "$sim_zip"
echo "Simulator app: $sim_zip"
