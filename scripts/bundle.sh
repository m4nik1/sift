#!/bin/sh
# Builds dist/Sift.app (release build, app icon, ad-hoc signed).
# `scripts/bundle.sh --install` also copies it to ~/Applications.
set -e
cd "$(dirname "$0")/.."
[ -f assets/AppIcon.icns ] || assets/make-icns.sh
cargo build --release
app=dist/Sift.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/sift "$app/Contents/MacOS/sift"
cp assets/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Sift</string>
  <key>CFBundleDisplayName</key><string>Sift</string>
  <key>CFBundleIdentifier</key><string>com.maniksoomro.sift</string>
  <key>CFBundleExecutable</key><string>sift</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$app" >/dev/null
echo "built $(pwd)/$app"
if [ "$1" = "--install" ]; then
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/Sift.app"
  cp -R "$app" "$HOME/Applications/Sift.app"
  echo "installed $HOME/Applications/Sift.app"
fi
