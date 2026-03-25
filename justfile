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

# Package release tarball
package version:
    #!/bin/sh
    set -e
    just build
    TRIPLE=$(rustc -vV | grep host | awk '{print $2}')
    cd target/release
    tar czf "caw-{{version}}-${TRIPLE}.tar.gz" caw
    echo "Packaged: caw-{{version}}-${TRIPLE}.tar.gz"

# Update Homebrew tap formula
update-homebrew version:
    #!/bin/sh
    set -e
    : "${GH_TOKEN:?GH_TOKEN required}"
    TRIPLE="aarch64-apple-darwin"
    URL="https://github.com/pablovilas/caw/releases/download/{{version}}/caw-{{version}}-${TRIPLE}.tar.gz"
    for i in 1 2 3 4 5; do
      SHA=$(curl -sL "$URL" | shasum -a 256 | awk '{print $1}')
      if [ "$SHA" != "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" ]; then
        break
      fi
      echo "Asset not ready yet, waiting..."
      sleep 10
    done
    echo "SHA256: $SHA"
    git clone "https://x-access-token:${GH_TOKEN}@github.com/pablovilas/homebrew-tap.git" tap
    cd tap
    cat > Formula/caw.rb << FORMULA
    class Caw < Formula
      desc "Monitor your AI coding assistants"
      homepage "https://github.com/pablovilas/caw"
      version "$(echo '{{version}}' | sed 's/^v//')"
      license "MIT"

      on_macos do
        url "${URL}"
        sha256 "${SHA}"
      end

      def install
        bin.install "caw"
      end

      test do
        assert_match "coding assistant watcher", shell_output("\#{bin}/caw --help")
      end
    end
    FORMULA
    git config user.name "github-actions[bot]"
    git config user.email "github-actions[bot]@users.noreply.github.com"
    git add Formula/caw.rb
    git commit -m "Update caw to {{version}}"
    git push

# Tag and push a release (runs CI first)
release version:
    @echo "Running checks before release..."
    just ci
    @echo "Tagging v{{version}}..."
    git tag "v{{version}}"
    git push origin "v{{version}}"
    @echo "Release v{{version}} pushed. GitHub Actions will build and publish."

# Run the app
run *args:
    cargo run -p caw -- {{args}}

# Set up git hooks
setup:
    git config core.hooksPath .githooks
