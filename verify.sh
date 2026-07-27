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
# `npm run typecheck`, no `npx tsc`: si typescript no esta instalado, npx sale 0
# sin typechequear nada (fallo enmascarado). El script resuelve el binario local
# o falla ruidoso.
npm run typecheck
npm test
npm run build

echo "TODO VERDE"
