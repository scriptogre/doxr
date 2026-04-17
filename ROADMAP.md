# Roadmap

## v0.1.0 ✓

- [x] CLI with MkDocs, Sphinx, and Rust-style `[Symbol]` syntax
- [x] Zero-config auto-detection of src layout and doc style
- [x] Symbol resolution: re-exports, inheritance, `self.x` attributes
- [x] External symbol validation via `objects.inv` inventories
- [x] Ruff-compatible output format
- [x] PyCharm plugin with Ctrl+Click, highlighting, and squiggles
- [x] Benchmarks
- [x] Publish to PyPI

## Next

- [ ] Inline suppression (`# drefs: ignore` or similar)
- [ ] GitHub Action for CI integration
- [ ] Publish PyCharm plugin to JetBrains Marketplace
- [ ] VS Code extension
- [ ] `.pyi` stub file resolution for third-party symbols

## Future

- [ ] Performance: sub-100ms on 500+ file projects
- [ ] Rename-aware refactoring in editor plugins
