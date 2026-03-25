#!/bin/sh
set -e

VERSION="${1:?Usage: update-homebrew.sh <version>}"
GH_TOKEN="${GH_TOKEN:?GH_TOKEN required}"

TRIPLE="aarch64-apple-darwin"
URL="https://github.com/pablovilas/caw/releases/download/${VERSION}/caw-${VERSION}-${TRIPLE}.tar.gz"

# Wait for release asset to be available
for i in 1 2 3 4 5; do
  SHA=$(curl -sL "$URL" | shasum -a 256 | awk '{print $1}')
  # Check it's not the SHA of an empty file
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
  version "${VERSION#v}"
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
git commit -m "Update caw to ${VERSION}"
git push
