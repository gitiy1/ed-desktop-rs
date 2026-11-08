#!/bin/sh
set -eu

if [ "$(uname -s)" != Darwin ]; then
    echo "macOS packaging requires macOS (codesign, ditto and hdiutil)." >&2
    exit 1
fi

release_dir=${1:-target/release}
binary="$release_dir/endao-desktop"
app="$release_dir/Endao Books.app"
dmg="$release_dir/endao-desktop-macos-arm64.dmg"
archive="$release_dir/endao-desktop-macos-arm64.app.zip"

if [ ! -x "$binary" ]; then
    echo "missing macOS release binary: $binary" >&2
    exit 1
fi

lipo -verify_arch arm64 "$binary"
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c '
import json, sys
metadata = json.load(sys.stdin)
print(next(package["version"] for package in metadata["packages"] if package["name"] == "endao-desktop"))
')

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/endao-desktop"
chmod 755 "$app/Contents/MacOS/endao-desktop"

python3 - "$app/Contents/Info.plist" "$version" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "wb") as output:
    plistlib.dump({
        "CFBundleDisplayName": "Endao Books",
        "CFBundleExecutable": "endao-desktop",
        "CFBundleIdentifier": "com.endao.books",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": "Endao Books",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": sys.argv[2].split("-")[0],
        "CFBundleVersion": sys.argv[2].split("-")[0],
        "LSMinimumSystemVersion": "12.0",
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
    }, output)
PY

printf 'APPL????' > "$app/Contents/PkgInfo"
plutil -lint "$app/Contents/Info.plist"

# Ad-hoc signing seals the bundle without requiring distribution credentials.
codesign --force --sign - --timestamp=none "$app"
codesign --verify --deep --strict --verbose=2 "$app"

# GitHub artifact uploads do not preserve executable permissions. A ZIP made
# with ditto preserves the .app bundle's permissions and resource metadata.
rm -f "$archive"
ditto -c -k --sequesterRsrc --keepParent "$app" "$archive"
unzip -tq "$archive"

staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT HUP INT TERM
ditto "$app" "$staging/Endao Books.app"
ln -s /Applications "$staging/Applications"
hdiutil create -volname "Endao Books" -srcfolder "$staging" -ov -format UDZO "$dmg"
hdiutil verify "$dmg"

file "$app/Contents/MacOS/endao-desktop"
echo "Created $app"
echo "Created $archive"
echo "Created $dmg"
