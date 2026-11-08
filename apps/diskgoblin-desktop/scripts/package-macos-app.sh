#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS is required for packaging.' >&2; exit 1; }
npm run build
if [[ -d /tmp/dg009-cargo-home ]]; then
  CARGO_HOME=/tmp/dg009-cargo-home cargo build --offline --manifest-path ../../Cargo.toml -p diskgoblin-cli --bin diskgoblin
else
  cargo build --manifest-path ../../Cargo.toml -p diskgoblin-cli --bin diskgoblin
fi
bundle="$PWD/dist-macos/DiskGoblin.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
# Replace only generated application assets, never the selected scan directory.
rsync -a --delete dist/ "$bundle/Contents/Resources/dist/"
cp ../../target/debug/diskgoblin "$bundle/Contents/MacOS/diskgoblin-engine"
/usr/bin/swift scripts/macos-icon.swift "$PWD/dist-macos"
/usr/bin/iconutil -c icns "$PWD/dist-macos/AppIcon.iconset" -o "$bundle/Contents/Resources/AppIcon.icns"
/usr/bin/swiftc scripts/macos-host.swift -framework Cocoa -framework WebKit -o "$bundle/Contents/MacOS/DiskGoblin"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>DiskGoblin</string>
<key>CFBundleIdentifier</key><string>com.diskgoblin.desktop</string>
<key>CFBundleName</key><string>DiskGoblin</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
printf 'Built %s\n' "$bundle"
