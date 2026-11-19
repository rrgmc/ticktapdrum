#!/usr/bin/env bash
# Builds TickTapDrum for macOS on Apple silicon (arm64) and packages it into dist/:
#   ticktapdrum-<version>-macos-arm64.dmg  (drag the app to Applications)
#   ticktapdrum-<version>-macos-arm64.zip  (the app alone)
#
# Signing and notarization are set by environment variables:
#   DM_MACOS_SIGN_IDENTITY   a Developer ID Application identity in the keychain, such as
#                            "Developer ID Application: Some Name (TEAMID)". Unset: the app is
#                            signed ad hoc, and the first start needs right-click > Open.
#   DM_MACOS_NOTARY_KEY      the path of an App Store Connect API key (.p8 file)
#   DM_MACOS_NOTARY_KEY_ID   that key's ID
#   DM_MACOS_NOTARY_ISSUER   that key's issuer ID
# With all three notary variables set (and an identity), Apple notarizes the app and the disk
# image, and the script staples the tickets to both.
# Usage, from the repository root:  bash apps/dm-desktop/packaging/macos/package.sh 0.2.0
set -euo pipefail
VERSION="$1"
HERE="$(cd "$(dirname "$0")" && pwd)"
ICON_PNG="$HERE/../../assets/icon-1024.png"
ZIP="dist/ticktapdrum-$VERSION-macos-arm64.zip"
DMG="dist/ticktapdrum-$VERSION-macos-arm64.dmg"

IDENTITY="${DM_MACOS_SIGN_IDENTITY:-}"
NOTARIZE=no
if [ -n "${DM_MACOS_NOTARY_KEY:-}" ] && [ -n "${DM_MACOS_NOTARY_KEY_ID:-}" ] && [ -n "${DM_MACOS_NOTARY_ISSUER:-}" ]; then
    if [ -z "$IDENTITY" ]; then
        echo "error: the notary variables are set but DM_MACOS_SIGN_IDENTITY is not; Apple notarizes only signed apps" >&2
        exit 1
    fi
    NOTARIZE=yes
elif [ -n "${DM_MACOS_NOTARY_KEY:-}${DM_MACOS_NOTARY_KEY_ID:-}${DM_MACOS_NOTARY_ISSUER:-}" ]; then
    echo "error: set all three of DM_MACOS_NOTARY_KEY, DM_MACOS_NOTARY_KEY_ID and DM_MACOS_NOTARY_ISSUER, or none" >&2
    exit 1
fi

# Sends a file to Apple's notary service and waits for the verdict. On a rejection it prints
# Apple's log, which names each problem, and stops the script.
notarize() {
    local file="$1" result id status
    result="$(mktemp)"
    xcrun notarytool submit "$file" \
        --key "$DM_MACOS_NOTARY_KEY" --key-id "$DM_MACOS_NOTARY_KEY_ID" --issuer "$DM_MACOS_NOTARY_ISSUER" \
        --wait --output-format json > "$result" || true
    cat "$result"
    echo
    id="$(plutil -extract id raw -o - "$result" 2>/dev/null || true)"
    status="$(plutil -extract status raw -o - "$result" 2>/dev/null || true)"
    rm -f "$result"
    if [ "$status" != "Accepted" ]; then
        echo "error: notarization of $file ended as '${status:-no answer}'" >&2
        if [ -n "$id" ]; then
            xcrun notarytool log "$id" \
                --key "$DM_MACOS_NOTARY_KEY" --key-id "$DM_MACOS_NOTARY_KEY_ID" --issuer "$DM_MACOS_NOTARY_ISSUER" >&2 || true
        fi
        exit 1
    fi
}

TARGET=aarch64-apple-darwin
cargo build --release --locked -p dm-desktop --target "$TARGET"

mkdir -p dist
rm -f "$ZIP" "$DMG"
STAGE="$(mktemp -d)"

# The icon: an .icns made from the rounded desktop icon (written by tools/icons).
ICONSET="$STAGE/icon.iconset"
mkdir -p "$ICONSET"
for SIZE in 16 32 128 256 512; do
    sips -z "$SIZE" "$SIZE" "$ICON_PNG" --out "$ICONSET/icon_${SIZE}x${SIZE}.png" > /dev/null
    DOUBLE=$((SIZE * 2))
    sips -z "$DOUBLE" "$DOUBLE" "$ICON_PNG" --out "$ICONSET/icon_${SIZE}x${SIZE}@2x.png" > /dev/null
done
iconutil -c icns "$ICONSET" -o "$STAGE/icon.icns"

# The app bundle. It sits in its own folder, which becomes the disk image.
# macOS wants the bundle version as plain numbers, so a pre-release such as 0.2.0-rc.1 gives 0.2.0.
DMG_ROOT="$STAGE/dmg"
APP="$DMG_ROOT/TickTapDrum.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# The program inside takes the capitalized name: the app menu ("Hide TickTapDrum", "Quit
# TickTapDrum") uses the program's name.
cp "target/$TARGET/release/ticktapdrum" "$APP/Contents/MacOS/TickTapDrum"
cp "$STAGE/icon.icns" "$APP/Contents/Resources/icon.icns"
sed "s/@VERSION@/${VERSION%%-*}/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"
# The app's name in each language Info.plist lists.
cp -R "$HERE/en.lproj" "$HERE/pt-BR.lproj" "$APP/Contents/Resources/"

# Signing goes from the inside out: first the program, then the bundle around it.
if [ -n "$IDENTITY" ]; then
    # The hardened runtime is what notarization asks for. The app needs no entitlements: it plays
    # audio but records nothing, and it runs no generated code.
    for CODE in "$APP/Contents/MacOS/TickTapDrum" "$APP"; do
        codesign --force --options runtime --timestamp --sign "$IDENTITY" "$CODE"
    done
else
    # Apple silicon refuses to run unsigned code; an ad hoc signature is enough to start it.
    echo "DM_MACOS_SIGN_IDENTITY is not set: signing ad hoc"
    for CODE in "$APP/Contents/MacOS/TickTapDrum" "$APP"; do
        codesign --force --sign - "$CODE"
    done
fi
codesign --verify --strict --verbose=2 "$APP"

# Notarize the app first and staple its ticket, so the zip and the disk image both carry an app
# that starts without a network check.
if [ "$NOTARIZE" = yes ]; then
    ditto -c -k --keepParent "$APP" "$STAGE/notarize.zip"
    notarize "$STAGE/notarize.zip"
    xcrun stapler staple "$APP"
else
    echo "The notary variables are not set: skipping notarization"
fi

ditto -c -k --keepParent "$APP" "$ZIP"
ln -s /Applications "$DMG_ROOT/Applications"
hdiutil create -volname "TickTapDrum" -srcfolder "$DMG_ROOT" -ov -format UDZO "$DMG"

if [ -n "$IDENTITY" ]; then
    codesign --force --timestamp --sign "$IDENTITY" "$DMG"
    codesign --verify --strict --verbose=2 "$DMG"
fi
if [ "$NOTARIZE" = yes ]; then
    notarize "$DMG"
    xcrun stapler staple "$DMG"
    xcrun stapler validate "$DMG"
fi

rm -rf "$STAGE"
ls -l dist
