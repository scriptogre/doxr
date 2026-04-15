# doxr

[![CI](https://github.com/scriptogre/doxr/actions/workflows/ci.yml/badge.svg)](https://github.com/scriptogre/doxr/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/doxr.svg)](https://pypi.org/project/doxr/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An extremely fast Python docstring cross-reference checker, written in Rust.

<p align="center">
  <picture align="center">
    <source media="(prefers-color-scheme: dark)" srcset="assets/benchmark-dark-v3.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/benchmark-light-v3.svg">
    <img alt="Shows a bar chart with benchmark results." src="assets/benchmark-light-v3.svg">
  </picture>
</p>

<p align="center">
  <i>Validating cross-references in <a href="https://github.com/tinygrad/tinygrad">tinygrad</a> (697 Python files). <a href="BENCHMARKS.md">~460x faster.</a></i>
</p>

## Usage

```bash
uvx doxr .
```

```
src/my_pkg/models.py:12:5: DXR001 Unresolved reference `my_pkg.old_module.Foo`
src/my_pkg/views.py:45:9: DXR001 Unresolved reference `Nonexistent`
Found 2 errors.
```

That's it. No config needed. Auto-detects your src layout and doc style.

## Supported syntax

- MkDocs: `[text][pkg.mod.Class]`, `[pkg.mod.Class][]`
- Sphinx: `` :class:`pkg.mod.Class` ``
- Rust-style: `[Symbol]`, `` [`Symbol`] ``, `[pkg.mod.Class]`

`[User]` resolves via the current file's imports. `[pkg.models.User]` resolves directly. Escape with `\[not a ref\]`.

doxr understands `__init__.py` re-exports, inheritance chains, and `self.x` attributes.

## Configuration

Optional. Add to `pyproject.toml`:

```toml
[tool.doxr]
src = ["src"]                  # auto-detected if omitted
style = "auto"                 # "mkdocs" | "sphinx" | "auto"
inventories = [                # validate against external symbols
    "https://docs.python.org/3/objects.inv",
]
```

## Editor support

### PyCharm / IntelliJ

- Ctrl+Click on each segment of a dotted path
- Syntax highlighting on cross-references
- Red squiggles on broken references

Install from `editors/pycharm/build/distributions/doxr-pycharm-*.zip` via Settings > Plugins > Install from Disk.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
