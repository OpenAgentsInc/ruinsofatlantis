#pragma once
#include "draco_decoder/src/ffi.rs.h"
#include "rust/cxx.h"
#include <cstdint>
#include <vector>

rust::Vec<uint8_t> decode_point_cloud(rust::Slice<const uint8_t> data);

size_t decode_mesh_direct_write(const uint8_t *data, size_t data_len,
                                uint8_t *out_ptr, size_t out_len);

size_t debug_mesh_buffer_len(const uint8_t *data, size_t data_len);
