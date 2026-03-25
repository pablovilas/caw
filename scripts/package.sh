#!/bin/sh
set -e

VERSION="${1:?Usage: package.sh <version>}"
TRIPLE=$(rustc -vV | grep host | awk '{print $2}')

cargo build --release -p caw

cd target/release
tar czf "caw-${VERSION}-${TRIPLE}.tar.gz" caw
echo "Packaged: caw-${VERSION}-${TRIPLE}.tar.gz"
