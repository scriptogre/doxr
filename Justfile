# Build (release)
build:
    cargo build --release

# Run drefs on a path
run *args:
    cargo run -- {{args}}

# Run all checks (fmt, clippy, tests, pattern-sync)
check:
    cargo fmt --check
    cargo clippy -- -D warnings
    cargo test
    ./scripts/check_pattern_sync.sh

# Run tests
test:
    cargo test

# Format code
fmt:
    cargo fmt

# Auto-fix clippy warnings
fix:
    cargo clippy --fix --allow-dirty

# Release a new version
release version:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! "{{version}}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "Error: version must be semver (e.g. 0.2.0), got '{{version}}'"
        exit 1
    fi
    if [ -n "$(git status --porcelain)" ]; then
        echo "Error: working tree is dirty. Commit or stash changes first."
        exit 1
    fi
    just check
    sed -i '' 's/^version = ".*"/version = "{{version}}"/' Cargo.toml
    cargo check --quiet 2>/dev/null
    git add Cargo.toml Cargo.lock
    git commit -m "Release v{{version}}"
    git tag "v{{version}}"
    echo ""
    echo "Ready to publish. Run:"
    echo "  git push && git push --tags"
