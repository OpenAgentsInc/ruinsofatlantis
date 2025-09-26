#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 <example_name>"
    exit 1
fi

EXAMPLE_NAME=$1

OUTPUT_DIR="examples/wasm/target"
TARGET_PLATFORM="wasm32-unknown-unknown"
WASM_BINDGEN_TARGET="web"

mkdir -p "$OUTPUT_DIR"

echo "Building example '$EXAMPLE_NAME' for target '$TARGET_PLATFORM'..."
cargo build --target "$TARGET_PLATFORM" --example "$EXAMPLE_NAME" --release 

if [ $? -ne 0 ]; then
    echo "Build failed for example '$EXAMPLE_NAME'."
    exit 1
fi

WASM_FILE="target/$TARGET_PLATFORM/release/examples/$EXAMPLE_NAME.wasm"
CUSTOM_OUTPUT_NAME="wasm_example"

echo "Processing wasm file with wasm-bindgen..."
wasm-bindgen \
    "$WASM_FILE" \
    --out-dir "$OUTPUT_DIR" \
    --out-name "$CUSTOM_OUTPUT_NAME" \
    --target "$WASM_BINDGEN_TARGET"

if [ $? -ne 0 ]; then
    echo "wasm-bindgen processing failed for '$EXAMPLE_NAME'."
    exit 1
fi

ASSETS_SOURCE_DIR="assets"
ASSETS_TARGET_DIR="examples/wasm/assets"

echo "Syncing assets to wasm output directory..."
rm -rf "$ASSETS_TARGET_DIR"
mkdir -p "$ASSETS_TARGET_DIR"
cp -r "$ASSETS_SOURCE_DIR/"* "$ASSETS_TARGET_DIR/"

echo "Build and processing completed successfully!"
echo "Output files are located in '$OUTPUT_DIR' with prefix '$CUSTOM_OUTPUT_NAME'."
