# doxr

[![CI](https://github.com/scriptogre/doxr/actions/workflows/ci.yml/badge.svg)](https://github.com/scriptogre/doxr/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/doxr.svg)](https://pypi.org/project/doxr/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An extremely fast Python docstring cross-reference checker, written in Rust.

```bash
$ uvx doxr .
src/my_pkg/models.py:12:5: DXR001 Unresolved reference `my_pkg.old_module.Foo`
src/my_pkg/views.py:45:9: DXR001 Unresolved reference `Nonexistent`
Found 2 errors.
```

## Highlights

- Validates against source code directly, **no docs build** required
- **Zero config**, auto-detects src layout and doc style
- Supports MkDocs, Sphinx, and Rust-style `[Symbol]` syntax
- Resolves re-exports, inheritance, and `self.x` attributes
- Ruff-compatible output format (`file:line:col: DXR001 message`)
- PyCharm plugin with **Ctrl+Click** navigation and red squiggles

## Installation

```bash
# Run without installing (recommended)
uvx doxr .

# Or install globally
uv tool install doxr

# Or via pip
pip install doxr
```

## Configuration

doxr works out of the box. Optionally, add to your `pyproject.toml`:

```toml
[tool.doxr]
src = ["src"]                  # auto-detected if omitted
style = "auto"                 # "mkdocs" | "sphinx" | "auto"
inventories = [                # external symbol validation
    "https://docs.python.org/3/objects.inv",
]
```

## Cross-Reference Syntax

doxr supports three syntax families:

| Syntax | Style |
|---|---|
| `[text][pkg.mod.Class]`, `[pkg.mod.Class][]` | MkDocs |
| `` :class:`pkg.mod.Class` `` | Sphinx |
| `[Symbol]`, `` [`Symbol`] ``, `[pkg.mod.Class]` | Rust-style intra-doc links |

The `[Symbol]` syntax follows Rust's intra-doc links. `[User]` resolves via the current file's imports. `[pkg.models.User]` resolves directly. Escape with `\[not a ref\]`.

## Editor Support

### PyCharm / IntelliJ

The doxr PyCharm plugin provides:
- Per-segment **Ctrl+Click** navigation on dotted paths (just like import statements)
- **Syntax highlighting** on cross-reference paths
- **Red squiggles** on unresolved references

Install from `editors/pycharm/build/distributions/doxr-pycharm-*.zip` via Settings > Plugins > Install from Disk.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

MIT
