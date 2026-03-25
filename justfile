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

# Run the app (tray mode)
run *args:
    cargo run -p caw -- {{args}}

# Set up git hooks
setup:
    git config core.hooksPath .githooks
