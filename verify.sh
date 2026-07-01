#!/usr/bin/env bash
# Puerta única: dev y CI corren esto mismo. Solo importa el exit code.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

echo "== cf-core (workspace raíz) =="
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

echo "== src-tauri (workspace propio, desacoplado a propósito) =="
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
cd ..

echo "== frontend =="
npx tsc --noEmit
npm run build

echo "TODO VERDE"
