#!/bin/sh
# Builds the Node.js package in livr-wasm/pkg (needs wasm-bindgen-cli of the
# version of the wasm-bindgen crate: cargo install wasm-bindgen-cli --version 0.2.129)
set -e
cd "$(dirname "$0")/.."
cargo build -p livr-wasm --target wasm32-unknown-unknown --profile wasm
wasm-bindgen target/wasm32-unknown-unknown/wasm/livr_wasm.wasm --target nodejs --out-dir livr-wasm/pkg
ls -l livr-wasm/pkg/livr_wasm_bg.wasm
