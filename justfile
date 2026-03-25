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
