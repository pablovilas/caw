# Run all CI checks locally
ci: lint test

# Lint with clippy
lint:
    cargo clippy --workspace -- -D warnings

# Run all tests
test:
    cargo test --workspace

# Build release binary
build:
    cargo build --release -p caw

# Generate .icns from SVG (requires resvg + iconutil)
iconset:
    #!/bin/sh
    set -e
    rm -rf /tmp/caw.iconset && mkdir -p /tmp/caw.iconset
    SVG="docs/svg/badge.svg"
    for size in 16 32 64 128 256 512 1024; do
      resvg "$SVG" "/tmp/caw.iconset/tmp_${size}.png" -w "$size" -h "$size"
    done
    cp /tmp/caw.iconset/tmp_16.png   /tmp/caw.iconset/icon_16x16.png
    cp /tmp/caw.iconset/tmp_32.png   /tmp/caw.iconset/icon_16x16@2x.png
    cp /tmp/caw.iconset/tmp_32.png   /tmp/caw.iconset/icon_32x32.png
    cp /tmp/caw.iconset/tmp_64.png   /tmp/caw.iconset/icon_32x32@2x.png
    cp /tmp/caw.iconset/tmp_128.png  /tmp/caw.iconset/icon_128x128.png
    cp /tmp/caw.iconset/tmp_256.png  /tmp/caw.iconset/icon_128x128@2x.png
    cp /tmp/caw.iconset/tmp_256.png  /tmp/caw.iconset/icon_256x256.png
    cp /tmp/caw.iconset/tmp_512.png  /tmp/caw.iconset/icon_256x256@2x.png
    cp /tmp/caw.iconset/tmp_512.png  /tmp/caw.iconset/icon_512x512.png
    cp /tmp/caw.iconset/tmp_1024.png /tmp/caw.iconset/icon_512x512@2x.png
    rm /tmp/caw.iconset/tmp_*.png
    iconutil -c icns /tmp/caw.iconset -o crates/caw/icons/caw.icns
    echo "Generated crates/caw/icons/caw.icns"

# Create macOS .app bundle
bundle: build
    #!/bin/sh
    set -e
    APP="target/release/caw.app"
    rm -rf "$APP"
    mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
    cp target/release/caw "$APP/Contents/MacOS/"
    cp crates/caw/icons/caw.icns "$APP/Contents/Resources/"
    cp crates/caw/macos/Info.plist "$APP/Contents/"
    echo "Built $APP"

# Test release locally (no publish)
release-dry-run:
    goreleaser release --clean --snapshot

# Tag and push a release (runs CI first)
release version:
    @echo "Running checks before release..."
    just ci
    @echo "Tagging v{{version}}..."
    git tag "v{{version}}"
    git push origin "v{{version}}"
    @echo "Release v{{version}} pushed. GoReleaser will build and publish."

# Run the app
run *args:
    cargo run -p caw -- {{args}}

# Set up git hooks
setup:
    git config core.hooksPath .githooks
