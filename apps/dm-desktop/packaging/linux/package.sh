#!/usr/bin/env bash
# Builds TickTapDrum for Linux (x86_64) and packages it into dist/:
#   ticktapdrum-<version>-linux-x86_64.tar.gz  (the program, its icon and a desktop file)
#   ticktapdrum_<version>_amd64.deb            (for Debian and Ubuntu; inside, a pre-release such
#                                                as 0.2.0-rc.1 takes the Debian version 0.2.0~rc.1)
# Build on the oldest system the app should run on (the release uses Ubuntu 22.04), since the
# program links against that system's C library.
# The build needs the ALSA headers (libasound2-dev on Debian and Ubuntu) for the audio output.
# The .deb needs dpkg-deb; dpkg-shlibdeps (package dpkg-dev) is used when it is there.
# Usage, from the repository root:  bash apps/dm-desktop/packaging/linux/package.sh 0.2.0
set -euo pipefail
VERSION="$1"
HERE="$(cd "$(dirname "$0")" && pwd)"
ASSETS="$HERE/../../assets"
BIN=target/release/ticktapdrum

cargo build --release --locked -p dm-desktop
mkdir -p dist
STAGE="$(mktemp -d)"

# The tarball.
TAR="$STAGE/ticktapdrum-$VERSION"
mkdir -p "$TAR"
cp "$BIN" "$TAR/"
cp "$ASSETS/icon-256.png" "$TAR/ticktapdrum.png"
cp "$HERE/ticktapdrum.desktop" "$TAR/"
tar -C "$STAGE" -czf "dist/ticktapdrum-$VERSION-linux-x86_64.tar.gz" "ticktapdrum-$VERSION"

# The Debian package. A Debian version sorts "~" before anything, so 0.2.0~rc.1 comes before 0.2.0.
# The file name keeps the plain version: GitHub turns "~" into "." in uploaded file names, so a "~"
# there would not match the name in SHA256SUMS.txt.
DEB_VERSION="$(printf '%s' "$VERSION" | sed 's/-/~/g')"
DEB="dist/ticktapdrum_${VERSION}_amd64.deb"
ROOT="$STAGE/deb"
install -D -m 0755 "$BIN" "$ROOT/usr/bin/ticktapdrum"
install -D -m 0644 "$HERE/ticktapdrum.desktop" "$ROOT/usr/share/applications/ticktapdrum.desktop"
install -D -m 0644 "$ASSETS/icon-256.png" "$ROOT/usr/share/icons/hicolor/256x256/apps/ticktapdrum.png"
strip "$ROOT/usr/bin/ticktapdrum"

# The libraries the program links against, as dpkg-shlibdeps finds them. It wants a source
# package description to read, so it runs from a scratch folder that has one.
LINKED="libc6, libgcc-s1, libasound2"
if command -v dpkg-shlibdeps > /dev/null; then
    SHLIBS="$STAGE/shlibs"
    mkdir -p "$SHLIBS/debian"
    printf 'Source: ticktapdrum\n\nPackage: ticktapdrum\nArchitecture: amd64\n' > "$SHLIBS/debian/control"
    FOUND="$(cd "$SHLIBS" && dpkg-shlibdeps -O "$ROOT/usr/bin/ticktapdrum" | sed -n 's/^shlibs:Depends=//p')"
    if [ -n "$FOUND" ]; then LINKED="$FOUND"; fi
fi
# The window, keyboard and graphics libraries are opened while the program runs, so
# dpkg-shlibdeps cannot see them. Graphics go through Vulkan, or OpenGL ES through EGL.
LOADED="libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libx11-6, libxcursor1, libxrandr2, libxi6, libvulkan1 | libegl1"

mkdir -p "$ROOT/DEBIAN"
cat > "$ROOT/DEBIAN/control" << EOF
Package: ticktapdrum
Version: $DEB_VERSION
Architecture: amd64
Maintainer: TickTapDrum project
Installed-Size: $(du -sk --exclude=DEBIAN "$ROOT" | cut -f1)
Depends: $LINKED, $LOADED
Section: sound
Priority: optional
Description: TickTapDrum
 Program drum patterns on a step grid and play them with General MIDI
 drum kits from a SoundFont bank.
EOF
find "$ROOT" -type d -exec chmod 0755 {} +
chmod 0644 "$ROOT/DEBIAN/control"
dpkg-deb --build --root-owner-group "$ROOT" "$DEB"
dpkg-deb --info "$DEB"
dpkg-deb --contents "$DEB"

rm -rf "$STAGE"
ls -l dist
