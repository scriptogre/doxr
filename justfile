# Development
check:
    cargo fmt --check
    cargo clippy -- -D warnings
    cargo test
    ./scripts/check_pattern_sync.sh

fmt:
    cargo fmt

fix:
    cargo clippy --fix --allow-dirty

# Run drefs on a fixture or path
run *args:
    cargo run -- {{args}}

# Release
release version:
    #!/usr/bin/env bash
    set -euo pipefail

    # Validate version format
    if [[ ! "{{version}}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "Error: version must be semver (e.g. 0.2.0), got '{{version}}'"
        exit 1
    fi

    # Ensure clean working tree
    if [ -n "$(git status --porcelain)" ]; then
        echo "Error: working tree is dirty. Commit or stash changes first."
        exit 1
    fi

    # Ensure all checks pass
    just check

    # Bump version in Cargo.toml
    sed -i '' 's/^version = ".*"/version = "{{version}}"/' Cargo.toml
    cargo check --quiet 2>/dev/null  # regenerate Cargo.lock

    # Commit and tag
    git add Cargo.toml Cargo.lock
    git commit -m "Release v{{version}}"
    git tag "v{{version}}"

    echo ""
    echo "Ready to publish. Run:"
    echo "  git push && git push --tags"
