# Installation

Install the published Python package from PyPI:

```bash
pip install openpit
```

The package requires Python 3.10 or newer.

## Local development install

From a checkout of the monorepo, install the Python bindings package:

```bash
pip install ./bindings/python
```

For native-extension development, use Maturin:

```bash
maturin develop --manifest-path bindings/python/Cargo.toml
```

## Documentation build

Documentation dependencies are isolated from runtime package dependencies:

```bash
cd bindings/python
pip install -r docs/requirements.txt
pip install .
sphinx-build -b html docs/ docs/_build -W
```
