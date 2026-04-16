#!/usr/bin/env bash
# Verify that cross-reference patterns in Rust and Kotlin stay in sync.
#
# Run from project root: ./scripts/check_pattern_sync.sh
# Called by CI — see .github/workflows/ci.yml

set -euo pipefail

RUST_FILE="src/patterns.rs"
KOTLIN_FILE="editors/pycharm/src/main/kotlin/com/doxr/intellij/DoxrPatterns.kt"

errors=0

# Extract raw regex strings from each file and normalize to the same
# escape format. Rust uses raw strings r"...", Kotlin uses "\\...".
# We normalize Kotlin's doubled backslashes to single backslashes.

extract_rust_patterns() {
    sed -n 's/.*Regex::new(r"\(.*\)").*/\1/p' "$RUST_FILE" | sort
}

extract_kotlin_patterns() {
    # Join continuation lines (Pattern.compile(\n "...")) into single lines,
    # then extract the pattern string and unescape doubled backslashes.
    tr '\n' '\f' < "$KOTLIN_FILE" \
        | sed 's/Pattern\.compile(\f *"/Pattern.compile("/g' \
        | tr '\f' '\n' \
        | sed -n 's/.*Pattern\.compile("\(.*\)").*/\1/p' \
        | sed 's/\\\\/\\/g' \
        | sort
}

rust_patterns=$(extract_rust_patterns)
kotlin_patterns=$(extract_kotlin_patterns)

if [ "$rust_patterns" != "$kotlin_patterns" ]; then
    echo "PATTERN SYNC ERROR: Regex patterns differ between Rust and Kotlin"
    echo ""
    echo "--- Rust (src/patterns.rs) ---"
    echo "$rust_patterns"
    echo ""
    echo "--- Kotlin (DoxrPatterns.kt) ---"
    echo "$kotlin_patterns"
    echo ""
    diff <(echo "$rust_patterns") <(echo "$kotlin_patterns") || true
    errors=$((errors + 1))
fi

# Check that both files have the same pattern names.
rust_names=$(grep -o 'MKDOCS_EXPLICIT\|MKDOCS_AUTOREF\|SPHINX_XREF\|RUST_STYLE' "$RUST_FILE" | sort -u)
kotlin_names=$(grep -o 'MKDOCS_EXPLICIT\|MKDOCS_AUTOREF\|SPHINX_XREF\|RUST_STYLE' "$KOTLIN_FILE" | sort -u)

if [ "$rust_names" != "$kotlin_names" ]; then
    echo "PATTERN SYNC ERROR: Pattern names differ between Rust and Kotlin"
    echo "Rust:   $rust_names"
    echo "Kotlin: $kotlin_names"
    errors=$((errors + 1))
fi

if [ "$errors" -gt 0 ]; then
    echo ""
    echo "Fix: update both src/patterns.rs and DoxrPatterns.kt to match."
    exit 1
fi

echo "Pattern sync OK: Rust and Kotlin patterns match."
