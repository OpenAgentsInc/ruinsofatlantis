use crate::MeshDecodeConfig;
use std::time::Instant;

#[cxx::bridge]
mod cpp {

    unsafe extern "C++" {
        include!("draco_decoder/include/decoder_api.h");

        pub fn decode_point_cloud(data: &[u8]) -> Vec<u8>;

        pub unsafe fn decode_mesh_direct_write(
            data: *const u8,
            data_len: usize,
            out_ptr: *mut u8,
            out_len: usize,
        ) -> usize;

        pub unsafe fn debug_mesh_buffer_len(data: *const u8, data_len: usize) -> usize;
    }
}

#[allow(dead_code)]
pub fn decode_point_cloud_native(data: &[u8]) -> Vec<u8> {
    cpp::decode_point_cloud(data)
}

pub async fn decode_mesh_native(data: &[u8], config: &MeshDecodeConfig) -> Option<Vec<u8>> {
    let start = Instant::now();
    let mut out_buf = vec![0u8; config.estimate_buffer_size()];
    let written = unsafe {
        cpp::decode_mesh_direct_write(
            data.as_ptr(),
            data.len(),
            out_buf.as_mut_ptr(),
            out_buf.len(),
        )
    };
    if written == 0 || written > out_buf.len() {
        return None;
    }
    out_buf.truncate(written);
    if std::env::var("RA_TRACE").map(|v| v == "1").unwrap_or(false) {
        eprintln!("decode_mesh_native took {:?}", start.elapsed());
    }

    Some(out_buf)
}

#[allow(dead_code)]
pub fn debug_estimate_draco_buffer_len(data: &[u8]) -> usize {
    unsafe { cpp::debug_mesh_buffer_len(data.as_ptr(), data.len()) }
}
