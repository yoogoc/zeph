#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script bundles the macOS development app." >&2
  exit 1
fi
cargo build
zeph_bundle_dir="target/debug/Zeph.app"
mkdir -p "$zeph_bundle_dir/Contents/MacOS" "$zeph_bundle_dir/Contents/Resources"
cp assets/app-icon/zeph.icns "$zeph_bundle_dir/Contents/Resources/zeph.icns"
cp target/debug/zeph "$zeph_bundle_dir/Contents/MacOS/zeph"
cat > "$zeph_bundle_dir/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>zeph</string>
<key>CFBundleIdentifier</key><string>dev.zeph.workbench</string>
<key>CFBundleName</key><string>Zeph</string>
<key>CFBundleDisplayName</key><string>Zeph</string>
<key>CFBundleIconFile</key><string>zeph.icns</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>0.0.4</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
printf 'Built %s\n' "$zeph_bundle_dir"
