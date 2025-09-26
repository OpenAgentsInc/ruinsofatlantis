# draco_decoder 

`draco_decoder` is a Rust library for decoding Draco compressed meshes. It provides native and WebAssembly (WASM) support with efficient bindings to the official Draco C++ library.

⚠️ Warning:
This crate currently only supports building on Linux and macOS platforms. Windows is not supported yet. and for the sake of building draco. cmake is required.

## Overview

- **Native:**  
  The native part uses [`cxx`](https://cxx.rs/) to create safe and ergonomic FFI bindings that directly connect to Draco's C++ decoding library. This allows efficient and zero-copy mesh decoding in native environments.

- **WASM:**  
  For WebAssembly targets, `draco_decoder` leverages the official Draco Emscripten build. It uses a JavaScript Worker to run the Draco decoder asynchronously, enabling non-blocking mesh decoding in the browser. The JavaScript implementation is available in a separate repository:  
  [https://github.com/jiangheng90/draco_decoder_js.git](https://github.com/jiangheng90/draco_decoder_js.git)

This design provides a unified Rust API while seamlessly switching between native and WASM implementations under the hood.


## native/wasm usage

```rust
use draco_decoder::{MeshDecodeConfig, AttributeDataType, decode_mesh};

// some async wrapper

let mut config = MeshDecodeConfig::new(vertex_count, index_count);

// Add attributes to decode (dimention and data type)
config.add_attribute(dim, AttributeDataType::Float32);
config.add_attribute(dim, AttributeDataType::Float32);

// Your Draco-encoded binary mesh data
let data: &[u8] = /* your Draco encoded data here */;

// Asynchronously decode the mesh data
let buf = decode_mesh(data, &config).await;

// wrapper end
```

## Performance

The performance of draco_decoder has been measured under different environments:
| Environment            | Typical Decoding Time |
| ---------------------- | --------------------- |
| Native (Release Build) | 3 ms – 7 ms           |
| WebAssembly (WASM)     | 30 ms – 50 ms         |



