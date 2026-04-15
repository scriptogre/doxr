# doxr Native Syntax Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Rust-style `[Symbol]` / `` [`Symbol`] `` intra-doc link syntax to both the doxr CLI and PyCharm plugin, with full short-name resolution via imports.

**Architecture:** Two-pass model. Pass 1 extracts references using regex (shared pattern between CLI and plugin). Pass 2 resolves — `FullyQualified` refs resolve directly, `ShortName` refs expand via module imports/definitions then resolve. CLI uses the symbol graph; plugin uses PyPsiFacade.

**Tech Stack:** Rust (CLI), Kotlin (PyCharm plugin), regex, tree-sitter-python, JetBrains PSI API.

**Spec:** `docs/superpowers/specs/2026-04-15-native-syntax-design.md`

---

### Task 1: Add `ReferenceKind` and native pattern to `extract.rs`

**Files:**
- Modify: `src/extract.rs`

- [ ] **Step 1: Add `ReferenceKind` enum and update `Reference` struct**

In `src/extract.rs`, add the enum above the `Reference` struct and add a `kind` field:

```rust
/// Whether a reference is fully qualified or a short name needing expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A dotted path like `pkg.models.User` — resolve directly.
    FullyQualified,
    /// A short name like `User` — expand via imports/definitions first.
    ShortName,
}
```

Update `Reference`:

```rust
pub struct Reference {
    pub target: String,
    pub offset: usize,
    pub kind: ReferenceKind,
}
```

- [ ] **Step 2: Fix all existing `Reference` construction sites to include `kind: ReferenceKind::FullyQualified`**

In `extract_mkdocs()` and `extract_sphinx()`, every `refs.push(Reference { ... })` needs `kind: ReferenceKind::FullyQualified` added. There are 3 sites total (2 in `extract_mkdocs`, 1 in `extract_sphinx`).

- [ ] **Step 3: Add the `DOXR_NATIVE` regex and `extract_native()` function**

Add the static regex:

```rust
// [identifier] or [`identifier`] — doxr-native (Rust-style intra-doc links)
// Negative lookbehind for ] (avoids MkDocs [text][path] second part) and \ (escaping).
// Negative lookahead for [ (avoids MkDocs [path][] first part).
static DOXR_NATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<![\\}\]])\[`?([a-zA-Z_][\w.]*)`?\](?!\[)").unwrap()
});
```

Note: Rust's `regex` crate does not support lookbehinds. We need to use a different approach — extract native refs last and filter out any whose offset overlaps with an already-extracted MkDocs/Sphinx ref.

Replace the static regex with a simpler pattern (no lookbehind):

```rust
// [identifier] or [`identifier`] — doxr-native (Rust-style intra-doc links)
// Negative lookahead for [ avoids MkDocs [path][] collisions.
// Overlap with MkDocs [text][path] second part is handled by deduplication.
static DOXR_NATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<!\\)\[`?([a-zA-Z_][\w.]*)`?\](?!\[)").unwrap()
});
```

Wait — Rust's `regex` crate also doesn't support `(?<!\\)`. We need a workaround. Use a capturing approach instead:

```rust
// [identifier] or [`identifier`] — doxr-native (Rust-style intra-doc links)
// Cannot use lookbehind (regex crate limitation), so we match an optional preceding
// char and filter in code.
static DOXR_NATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[`?([a-zA-Z_][\w.]*)`?\](?!\[)").unwrap()
});
```

Add the extraction function:

```rust
fn extract_native(content: &str, existing_offsets: &[usize]) -> Vec<Reference> {
    let mut refs = Vec::new();

    for cap in DOXR_NATIVE.captures_iter(content) {
        let full_match = cap.get(0).unwrap();

        // Skip if preceded by \ (escaped) or ] (MkDocs second bracket).
        let start = full_match.start();
        if start > 0 {
            let prev = content.as_bytes()[start - 1];
            if prev == b'\\' || prev == b']' {
                continue;
            }
        }

        if let Some(m) = cap.get(1) {
            let target = m.as_str().to_string();

            // Skip if this offset was already captured by MkDocs/Sphinx patterns.
            if existing_offsets.contains(&m.start()) {
                continue;
            }

            let kind = if target.contains('.') {
                ReferenceKind::FullyQualified
            } else {
                ReferenceKind::ShortName
            };

            refs.push(Reference {
                target,
                offset: m.start(),
                kind,
            });
        }
    }

    refs
}
```

- [ ] **Step 4: Wire `extract_native()` into `extract_references()`**

Update `extract_references` to call `extract_native` in all styles, passing existing offsets for dedup:

```rust
pub fn extract_references(content: &str, style: &DocStyle) -> Vec<Reference> {
    let mut refs = match style {
        DocStyle::Mkdocs => extract_mkdocs(content),
        DocStyle::Sphinx => extract_sphinx(content),
        DocStyle::Auto => {
            let mut r = extract_mkdocs(content);
            r.extend(extract_sphinx(content));
            r
        }
    };

    // Always extract doxr-native refs, deduplicating against existing offsets.
    let existing_offsets: Vec<usize> = refs.iter().map(|r| r.offset).collect();
    refs.extend(extract_native(content, &existing_offsets));

    refs
}
```

- [ ] **Step 5: Run `cargo build` to verify compilation**

Run: `cargo build`
Expected: compiles with no errors (there will be warnings about unused `kind` field in diagnostic.rs — that's expected, we use it in Task 3).

- [ ] **Step 6: Commit**

```bash
git add src/extract.rs
git commit -m "Add ReferenceKind and doxr-native pattern extraction"
```

---

### Task 2: Add unit tests for native pattern extraction

**Files:**
- Modify: `src/extract.rs` (tests module)

- [ ] **Step 1: Add unit tests for the new native syntax**

Add these tests to the existing `#[cfg(test)] mod tests` block in `src/extract.rs`:

```rust
#[test]
fn test_native_bare_brackets_fq() {
    let content = r#"See [pkg.models.User] for details."#;
    let refs = extract_references(content, &DocStyle::Auto);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target, "pkg.models.User");
    assert_eq!(refs[0].kind, ReferenceKind::FullyQualified);
}

#[test]
fn test_native_backtick_brackets_fq() {
    let content = r#"See [`pkg.models.User`] for details."#;
    let refs = extract_references(content, &DocStyle::Auto);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target, "pkg.models.User");
    assert_eq!(refs[0].kind, ReferenceKind::FullyQualified);
}

#[test]
fn test_native_short_name() {
    let content = r#"See [User] for details."#;
    let refs = extract_references(content, &DocStyle::Auto);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target, "User");
    assert_eq!(refs[0].kind, ReferenceKind::ShortName);
}

#[test]
fn test_native_short_name_backticks() {
    let content = r#"See [`User`] for details."#;
    let refs = extract_references(content, &DocStyle::Auto);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target, "User");
    assert_eq!(refs[0].kind, ReferenceKind::ShortName);
}

#[test]
fn test_native_escaped_ignored() {
    let content = r#"See \[User] for details."#;
    let refs = extract_references(content, &DocStyle::Auto);
    assert_eq!(refs.len(), 0);
}

#[test]
fn test_native_no_collision_with_mkdocs_explicit() {
    // [text][path] — only the MkDocs explicit ref should be extracted, not [text] or [path] natively.
    let content = r#"See [display text][pkg.models.User] for details."#;
    let refs = extract_references(content, &DocStyle::Mkdocs);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target, "pkg.models.User");
    assert_eq!(refs[0].kind, ReferenceKind::FullyQualified);
}

#[test]
fn test_native_no_collision_with_mkdocs_autoref() {
    // [path][] — only the MkDocs autoref should be extracted.
    let content = r#"See [pkg.models.User][] for details."#;
    let refs = extract_references(content, &DocStyle::Mkdocs);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target, "pkg.models.User");
    assert_eq!(refs[0].kind, ReferenceKind::FullyQualified);
}

#[test]
fn test_native_ignores_non_identifiers() {
    let content = r#"See [see above] and [1] and [some/path] for details."#;
    let refs = extract_references(content, &DocStyle::Auto);
    assert_eq!(refs.len(), 0);
}

#[test]
fn test_native_mixed_with_mkdocs_and_sphinx() {
    let content = r#"
    Native: [User]
    MkDocs: [text][pkg.models.Admin]
    Sphinx: :class:`pkg.models.User`
    Native FQ: [pkg.sub.helper_func]
    "#;
    let refs = extract_references(content, &DocStyle::Auto);
    assert_eq!(refs.len(), 4);
}

#[test]
fn test_native_underscore_start() {
    let content = r#"See [_private_func] for details."#;
    let refs = extract_references(content, &DocStyle::Auto);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target, "_private_func");
    assert_eq!(refs[0].kind, ReferenceKind::ShortName);
}
```

- [ ] **Step 2: Run the unit tests**

Run: `cargo test --lib`
Expected: all new tests pass, all existing tests still pass.

- [ ] **Step 3: Commit**

```bash
git add src/extract.rs
git commit -m "Add unit tests for doxr-native pattern extraction"
```

---

### Task 3: Add short name resolution to `diagnostic.rs`

**Files:**
- Modify: `src/diagnostic.rs`

- [ ] **Step 1: Add `expand_short_name` function and update `check()`**

Add a helper function to expand short names using the module's imports and definitions:

```rust
use crate::extract::ReferenceKind;
```

Add this function:

```rust
/// Expand a short name (e.g. `User`) to a fully-qualified path using the
/// module's imports and definitions. Returns `None` if the name isn't in scope.
fn expand_short_name(name: &str, module: &crate::graph::Module) -> Option<String> {
    // 1. Check imports.
    for imp in &module.imports {
        let local_name = imp.alias.as_deref().unwrap_or(&imp.name);
        if local_name == name {
            return Some(format!("{}.{}", imp.source, imp.name));
        }
    }

    // 2. Check local definitions.
    if module.definitions.contains_key(name) {
        return Some(format!("{}.{}", module.path, name));
    }

    None
}
```

- [ ] **Step 2: Update the `check()` function to handle `ShortName` refs**

In the `check()` function, update the ref processing loop. Replace the current block inside `for r in refs { ... }`:

```rust
for r in refs {
    if is_explicitly_skipped(&r, config) {
        continue;
    }

    // Expand short names to fully-qualified paths.
    let (target, is_short) = match r.kind {
        ReferenceKind::ShortName => {
            match expand_short_name(&r.target, module) {
                Some(fqn) => (fqn, true),
                None => {
                    // Short name not in scope — error.
                    diagnostics.push(Diagnostic {
                        file: file_path.clone(),
                        line: docstring.line,
                        col: docstring.col,
                        code: "DXR001",
                        message: format!("Unresolved reference `{}`", r.target),
                    });
                    continue;
                }
            }
        }
        ReferenceKind::FullyQualified => (r.target.clone(), false),
    };

    let internal = is_internal_target(&target, graph);

    if internal {
        if !graph.resolve(&target) {
            diagnostics.push(Diagnostic {
                file: file_path.clone(),
                line: docstring.line,
                col: docstring.col,
                code: "DXR001",
                message: format!(
                    "Unresolved reference `{}`",
                    if is_short { &r.target } else { &target }
                ),
            });
        }
    } else if inventory.covers_root(&target) {
        if !inventory.contains(&target) {
            diagnostics.push(Diagnostic {
                file: file_path.clone(),
                line: docstring.line,
                col: docstring.col,
                code: "DXR001",
                message: format!(
                    "Unresolved reference `{}`",
                    if is_short { &r.target } else { &target }
                ),
            });
        }
    }
}
```

Note: rename `is_internal` to `is_internal_target` to accept a `&str` target directly instead of a `Reference`:

```rust
fn is_internal_target(target: &str, graph: &SymbolGraph) -> bool {
    let root = target.split('.').next().unwrap_or("");
    graph.modules.keys().any(|path| {
        path == root || path.starts_with(&format!("{root}."))
    })
}
```

Keep the old `is_internal` function too (it's used by the existing code path) or refactor it to call `is_internal_target`:

```rust
fn is_internal(reference: &Reference, graph: &SymbolGraph) -> bool {
    is_internal_target(&reference.target, graph)
}
```

- [ ] **Step 3: Run `cargo build` to verify compilation**

Run: `cargo build`
Expected: compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/diagnostic.rs
git commit -m "Add short name resolution in diagnostic check"
```

---

### Task 4: Create test fixture and integration tests

**Files:**
- Create: `tests/fixtures/native_syntax/pyproject.toml`
- Create: `tests/fixtures/native_syntax/src/pkg/__init__.py`
- Create: `tests/fixtures/native_syntax/src/pkg/models.py`
- Create: `tests/fixtures/native_syntax/src/pkg/services.py`
- Modify: `tests/integration.rs`

- [ ] **Step 1: Create the fixture project config**

`tests/fixtures/native_syntax/pyproject.toml`:

```toml
[tool.doxr]
src = ["src"]
style = "auto"
```

- [ ] **Step 2: Create the fixture package with models**

`tests/fixtures/native_syntax/src/pkg/__init__.py`:

```python
"""Root package that re-exports from submodules."""

from pkg.models import User, Admin

__all__ = ["User", "Admin"]
```

`tests/fixtures/native_syntax/src/pkg/models.py`:

```python
"""Models module with classes and functions."""


class User:
    """A user."""

    role: str = "user"

    def __init__(self, name: str) -> None:
        self.name = name

    def greet(self) -> str:
        """Return a greeting."""
        return f"Hello, {self.name}!"


class Admin(User):
    """An admin user."""

    level: int = 1


def helper_func() -> None:
    """A helper function."""
    pass
```

- [ ] **Step 3: Create `services.py` with all native syntax test cases**

`tests/fixtures/native_syntax/src/pkg/services.py`:

```python
"""Services module — tests doxr-native cross-reference syntax."""

from pkg.models import User, Admin, helper_func


def native_fq_refs() -> None:
    """FQ bare brackets.

    [pkg.models.User]
    [`pkg.models.Admin`]
    [pkg.models.User.greet]
    [pkg.models.User.name]
    [pkg.models.User.role]
    """
    pass


def native_short_refs() -> None:
    """Short names resolved via imports.

    [User]
    [`User`]
    [Admin]
    [helper_func]
    """
    pass


def native_broken_refs() -> None:
    """Broken refs that should be flagged.

    [Nonexistent]
    [pkg.models.Fake]
    [`AlsoFake`]
    """
    pass


def native_ignored() -> None:
    """These should NOT be treated as refs.

    \\[User\\]
    [see above]
    [1]
    [some/path]
    """
    pass


def native_mixed() -> None:
    """Mixed native + MkDocs + Sphinx.

    Native: [User]
    MkDocs explicit: [click here][pkg.models.Admin]
    MkDocs autoref: [pkg.models.User][]
    Sphinx: :class:`pkg.models.User`
    Native FQ: [pkg.models.helper_func]
    """
    pass
```

- [ ] **Step 4: Add integration tests**

Add to `tests/integration.rs`:

```rust
// ---------------------------------------------------------------------------
// Native syntax (Rust-style intra-doc links)
// ---------------------------------------------------------------------------

#[test]
fn native_syntax_no_false_positives() {
    let (stdout, _stderr, _code) = run_doxr("native_syntax");
    let errors = extract_unresolved(&stdout);

    let must_resolve = vec![
        // FQ bare brackets
        "pkg.models.User",
        "pkg.models.Admin",
        "pkg.models.User.greet",
        "pkg.models.User.name",
        "pkg.models.User.role",
        // Short names (should NOT appear in errors — they resolve via imports)
        "User",
        "Admin",
        "helper_func",
        // FQ in mixed context
        "pkg.models.helper_func",
    ];

    for valid in &must_resolve {
        assert!(
            !errors.contains(&valid.to_string()),
            "False positive: `{valid}` was flagged but should resolve.\nAll errors: {errors:?}"
        );
    }
}

#[test]
fn native_syntax_catches_broken_refs() {
    let (stdout, _stderr, code) = run_doxr("native_syntax");
    let errors = extract_unresolved(&stdout);

    let expected_errors = vec![
        "Nonexistent",
        "pkg.models.Fake",
        "AlsoFake",
    ];

    for expected in &expected_errors {
        assert!(
            errors.contains(&expected.to_string()),
            "Expected error for `{expected}` but it was not flagged.\nAll errors: {errors:?}"
        );
    }

    assert_ne!(code, 0, "Should exit non-zero when errors found");
}

#[test]
fn native_syntax_error_count() {
    let (stdout, _stderr, _code) = run_doxr("native_syntax");
    let errors = extract_unresolved(&stdout);
    assert_eq!(
        errors.len(),
        3,
        "Expected exactly 3 errors, got {}.\nErrors: {errors:?}",
        errors.len()
    );
}
```

- [ ] **Step 5: Run the integration tests**

Run: `cargo test native_syntax`
Expected: all 3 new tests pass.

- [ ] **Step 6: Run ALL tests to verify no regressions**

Run: `cargo test`
Expected: all existing tests still pass alongside the new ones.

- [ ] **Step 7: Commit**

```bash
git add tests/fixtures/native_syntax/ tests/integration.rs
git commit -m "Add test fixture and integration tests for native syntax"
```

---

### Task 5: Verify existing tests still pass (regression check)

**Files:** None (verification only)

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: ALL tests pass — edge_cases, decorated_classes, and native_syntax.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`
Expected: no warnings related to the changed code.

- [ ] **Step 3: Commit any fixes if needed**

Only if clippy or tests surfaced issues.

---

### Task 6: Add native pattern to PyCharm plugin `DoxrReferenceProvider.kt`

**Files:**
- Modify: `editors/pycharm/src/main/kotlin/com/doxr/intellij/DoxrReferenceProvider.kt`

- [ ] **Step 1: Add `DOXR_NATIVE` pattern to the companion object**

In the `companion object` block, after the existing patterns:

```kotlin
// [identifier] or [`identifier`] — doxr-native (Rust-style intra-doc links)
private val DOXR_NATIVE = Pattern.compile("\\[`?([a-zA-Z_][\\w.]*)`?\\](?!\\[)")
```

- [ ] **Step 2: Add `findNativeRefs` method**

Add a new method to extract native refs, handling escaping and MkDocs dedup:

```kotlin
private fun findNativeRefs(
    content: String,
    contentOffset: Int,
    element: PyStringLiteralExpression,
    existingOffsets: Set<Int>,
    out: MutableList<PsiReference>,
) {
    val matcher = DOXR_NATIVE.matcher(content)
    while (matcher.find()) {
        // Skip if preceded by \ (escaped) or ] (MkDocs second bracket).
        val start = matcher.start()
        if (start > 0) {
            val prev = content[start - 1]
            if (prev == '\\' || prev == ']') continue
        }

        val path = matcher.group(1) ?: continue

        // Skip if this offset was already captured by MkDocs/Sphinx patterns.
        val pathStart = matcher.start(1)
        if (existingOffsets.contains(pathStart)) continue

        if (path.contains('.')) {
            // Fully qualified — same per-segment references as MkDocs/Sphinx.
            if (!path[0].isLowerCase() && path[0] != '_') continue
            createSegmentReferences(path, contentOffset + pathStart, element, out)
        } else {
            // Short name — single reference, resolved via file scope.
            val range = TextRange(contentOffset + pathStart, contentOffset + pathStart + path.length)
            out.add(DoxrPythonReference(element, range, path, resolveShort = true))
        }
    }
}
```

- [ ] **Step 3: Wire into `getReferencesByElement`**

Update `getReferencesByElement` to call `findNativeRefs` after existing extractors, passing offsets for dedup:

Replace the return statement area. After the Sphinx extraction and before `return references.toTypedArray()`, add:

```kotlin
// Collect offsets from MkDocs/Sphinx refs to avoid double-extracting.
val existingOffsets = references.mapNotNull { ref ->
    (ref as? DoxrPythonReference)?.rangeInElement?.startOffset
}.toSet()

// Extract doxr-native refs.
findNativeRefs(content, contentOffset, stringLiteral, existingOffsets, references)
```

- [ ] **Step 4: Commit**

```bash
git add editors/pycharm/src/main/kotlin/com/doxr/intellij/DoxrReferenceProvider.kt
git commit -m "Add doxr-native pattern to PyCharm reference provider"
```

---

### Task 7: Update `DoxrPythonReference.kt` for short name resolution

**Files:**
- Modify: `editors/pycharm/src/main/kotlin/com/doxr/intellij/DoxrPythonReference.kt`

- [ ] **Step 1: Add `resolveShort` parameter and short-name resolution**

Update the class to accept a `resolveShort` flag and resolve short names by looking up imports in the containing file:

```kotlin
class DoxrPythonReference(
    element: PyStringLiteralExpression,
    range: TextRange,
    private val qualifiedName: String,
    private val resolveShort: Boolean = false,
) : PsiReferenceBase<PyStringLiteralExpression>(element, range, /* soft = */ false) {

    override fun resolve(): PsiElement? {
        if (resolveShort) {
            return resolveShortName()
        }
        return resolveQualified()
    }

    private fun resolveShortName(): PsiElement? {
        val project = element.project
        val facade = PyPsiFacade.getInstance(project)
        val resolveContext = facade.createResolveContextFromFoothold(element)
        val withMembers = resolveContext.copyWithMembers()

        // Get the containing file to check its imports.
        val pyFile = element.containingFile as? PyFile ?: return null

        // 1. Check imports in the file.
        for (imp in pyFile.fromImports) {
            for (importedName in imp.importElements) {
                val visibleName = importedName.asName ?: importedName.importedQName?.lastComponent ?: continue
                if (visibleName == qualifiedName) {
                    val fqn = importedName.importedQName?.toString() ?: continue
                    val qName = QualifiedName.fromDottedString(fqn)
                    return facade.resolveQualifiedName(qName, withMembers).firstOrNull()
                        ?: resolveClassMember(facade, qName, withMembers)
                }
            }
        }

        // 2. Check definitions in the file.
        for (cls in pyFile.topLevelClasses) {
            if (cls.name == qualifiedName) return cls
        }
        for (func in pyFile.topLevelFunctions) {
            if (func.name == qualifiedName) return func
        }
        for (target in pyFile.topLevelAttributes) {
            if (target.name == qualifiedName) return target
        }

        return null
    }

    private fun resolveQualified(): PsiElement? {
        val project = element.project
        val facade = PyPsiFacade.getInstance(project)
        val qName = QualifiedName.fromDottedString(qualifiedName)
        val resolveContext = facade.createResolveContextFromFoothold(element)

        // Try as module/package first.
        val moduleResult = facade.resolveQualifiedName(qName, resolveContext)
            .asSequence()
            .filterIsInstance<PsiFileSystemItem>()
            .map { PyUtil.turnDirIntoInit(it) }
            .filterIsInstance<PyFile>()
            .firstOrNull()
        if (moduleResult != null) return moduleResult

        // Try as top-level function/class with member access.
        val withMembers = resolveContext.copyWithMembers()
        val directResult = facade.resolveQualifiedName(qName, withMembers).firstOrNull()
        if (directResult != null) return directResult

        // Try as a class method/attribute.
        return resolveClassMember(facade, qName, withMembers)
    }

    private fun resolveClassMember(
        facade: PyPsiFacade,
        qName: QualifiedName,
        withMembers: com.jetbrains.python.psi.resolve.PyQualifiedNameResolveContext,
    ): PsiElement? {
        if (qName.componentCount > 1) {
            val parentQName = qName.removeLastComponent()
            val parentResult = facade.resolveQualifiedName(parentQName, withMembers)
                .filterIsInstance<PyClass>()
                .firstOrNull()
            if (parentResult != null) {
                val method = parentResult.findMethodByName(qName.lastComponent, true, null)
                if (method != null) return method
                val attr = parentResult.findClassAttribute(qName.lastComponent.orEmpty(), true, null)
                if (attr != null) return attr
            }
        }
        return null
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add editors/pycharm/src/main/kotlin/com/doxr/intellij/DoxrPythonReference.kt
git commit -m "Add short name resolution to PyCharm reference"
```

---

### Task 8: Add native pattern to PyCharm `DoxrAnnotator.kt`

**Files:**
- Modify: `editors/pycharm/src/main/kotlin/com/doxr/intellij/DoxrAnnotator.kt`

- [ ] **Step 1: Add `DOXR_NATIVE` pattern to the companion object**

In the `companion object`, after the existing patterns:

```kotlin
private val DOXR_NATIVE = Pattern.compile("\\[`?([a-zA-Z_][\\w.]*)`?\\](?!\\[)")
```

- [ ] **Step 2: Add `highlightNativeRefs` method**

Add a method that highlights native refs — both FQ (per-segment) and short names (single highlight):

```kotlin
private fun highlightNativeRefs(
    content: String,
    contentOffset: Int,
    elementOffset: Int,
    existingOffsets: Set<Int>,
    holder: AnnotationHolder,
) {
    val matcher = DOXR_NATIVE.matcher(content)
    while (matcher.find()) {
        val start = matcher.start()
        if (start > 0) {
            val prev = content[start - 1]
            if (prev == '\\' || prev == ']') continue
        }

        val path = matcher.group(1) ?: continue
        val pathStart = matcher.start(1)

        if (existingOffsets.contains(pathStart)) continue

        if (path.contains('.')) {
            // FQ — must start with lowercase/underscore.
            if (!path[0].isLowerCase() && path[0] != '_') continue

            // Highlight each segment as identifier, dots as punctuation.
            val segments = path.split('.')
            var pos = pathStart
            for (segment in segments) {
                val absStart = elementOffset + contentOffset + pos
                val absEnd = absStart + segment.length
                holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
                    .range(TextRange(absStart, absEnd))
                    .textAttributes(DefaultLanguageHighlighterColors.IDENTIFIER)
                    .create()
                pos += segment.length + 1
            }
            pos = pathStart
            for (i in 0 until segments.size - 1) {
                pos += segments[i].length
                val dotStart = elementOffset + contentOffset + pos
                holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
                    .range(TextRange(dotStart, dotStart + 1))
                    .textAttributes(DefaultLanguageHighlighterColors.DOT)
                    .create()
                pos += 1
            }
        } else {
            // Short name — single identifier highlight.
            val absStart = elementOffset + contentOffset + pathStart
            val absEnd = absStart + path.length
            holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
                .range(TextRange(absStart, absEnd))
                .textAttributes(DefaultLanguageHighlighterColors.IDENTIFIER)
                .create()
        }
    }
}
```

- [ ] **Step 3: Wire into `annotate()`**

Update the `annotate()` method to call `highlightNativeRefs` after existing highlights, with offset dedup. Collect offsets from existing highlights by tracking them. The simplest approach: call `highlightNativeRefs` with the offsets from the existing MkDocs/Sphinx patterns.

Replace the end of the `annotate()` method:

```kotlin
override fun annotate(element: PsiElement, holder: AnnotationHolder) {
    val stringLiteral = element as? PyStringLiteralExpression ?: return
    val text = stringLiteral.text
    if (!text.startsWith("\"\"\"") && !text.startsWith("'''")) return

    val content = stringLiteral.stringValue
    val contentOffset = stringLiteral.stringValueTextRanges.firstOrNull()?.startOffset ?: return
    val elementOffset = stringLiteral.textRange.startOffset

    // Track offsets from MkDocs/Sphinx patterns for dedup.
    val existingOffsets = mutableSetOf<Int>()

    collectOffsets(MKDOCS_EXPLICIT, content, 1, existingOffsets)
    collectOffsets(MKDOCS_AUTOREF, content, 1, existingOffsets)
    collectOffsets(SPHINX_XREF, content, 2, existingOffsets)

    highlightRefs(MKDOCS_EXPLICIT, content, 1, contentOffset, elementOffset, holder)
    highlightRefs(MKDOCS_AUTOREF, content, 1, contentOffset, elementOffset, holder)
    highlightRefs(SPHINX_XREF, content, 2, contentOffset, elementOffset, holder)

    highlightNativeRefs(content, contentOffset, elementOffset, existingOffsets, holder)
}

private fun collectOffsets(
    pattern: Pattern,
    content: String,
    group: Int,
    out: MutableSet<Int>,
) {
    val matcher = pattern.matcher(content)
    while (matcher.find()) {
        val m = matcher.group(group) ?: continue
        var path = m
        val tildeOffset = if (path.startsWith("~")) 1 else 0
        if (tildeOffset > 0) path = path.substring(1)
        if (!path.contains('.')) continue
        if (!path[0].isLowerCase() && path[0] != '_') continue
        out.add(matcher.start(group) + tildeOffset)
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add editors/pycharm/src/main/kotlin/com/doxr/intellij/DoxrAnnotator.kt
git commit -m "Add doxr-native pattern to PyCharm annotator"
```

---

### Task 9: Build and verify the PyCharm plugin

**Files:** None (verification only)

- [ ] **Step 1: Build the plugin**

Run: `cd /Users/chris/Projects/doxr/editors/pycharm && ./gradlew buildPlugin`
Expected: BUILD SUCCESSFUL. Plugin zip at `build/distributions/doxr-pycharm-*.zip`.

- [ ] **Step 2: Commit version bump if needed**

If the plugin version needs bumping in `build.gradle.kts`, update it and commit.

---

### Task 10: Final full verification

**Files:** None (verification only)

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test`
Expected: ALL tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`
Expected: no warnings.

- [ ] **Step 3: Verify PyCharm plugin builds**

Run: `cd /Users/chris/Projects/doxr/editors/pycharm && ./gradlew buildPlugin`
Expected: BUILD SUCCESSFUL.
