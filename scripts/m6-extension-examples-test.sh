#!/usr/bin/env sh
# Claimed-language M6 extension contract, determinism, and isolation gate.
set -eu

cargo test -p crowd-core --test m6_extensions
python3 -m unittest -q tests/test_m6_extensions.py tests/test_m6_extension_examples.py
