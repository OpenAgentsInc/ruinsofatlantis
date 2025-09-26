#[cfg(not(target_arch = "wasm32"))]
mod ffi;
pub mod utils;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
use ffi::decode_mesh_native;
pub use utils::{AttributeDataType, AttributeValues, MeshAttribute, MeshDecodeConfig};
#[cfg(target_arch = "wasm32")]
use wasm::decode_mesh_wasm_worker;

#[cfg(not(target_arch = "wasm32"))]
pub async fn decode_mesh(data: &[u8], config: &MeshDecodeConfig) -> Option<Vec<u8>> {
    decode_mesh_native(data, config).await
}

#[cfg(target_arch = "wasm32")]
pub async fn decode_mesh(data: &[u8], config: &MeshDecodeConfig) -> Option<Vec<u8>> {
    decode_mesh_wasm_worker(data, config).await
}

#[cfg(test)]
mod tests {

    #[cfg(not(target_arch = "wasm32"))]
    use super::ffi::{debug_estimate_draco_buffer_len, decode_point_cloud_native};
    use super::utils::{AttributeDataType, MeshDecodeConfig};
    use crate::decode_mesh;
    use std::collections::HashSet;
    use std::fs::{self};

    fn quantize(v: &[f32]) -> [i32; 3] {
        [
            (v[0] * 1000.0).round() as i32,
            (v[1] * 1000.0).round() as i32,
            (v[2] * 1000.0).round() as i32,
        ]
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_decode_point_cloud() {
        let input = fs::read("assets/pointcloud.drc").expect("Failed to read pointcloud.drc");
        let output = decode_point_cloud_native(&input);

        assert!(
            output.len() % 12 == 0,
            "Expected output to be a multiple of 12 bytes (3 floats per point)"
        );

        let floats: Vec<f32> = output
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();

        let actual: HashSet<[i32; 3]> = floats.chunks_exact(3).map(quantize).collect();

        let expected: HashSet<[i32; 3]> = [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [2.0, 2.0, 2.0]]
            .iter()
            .map(|v| quantize(v))
            .collect();

        assert_eq!(
            actual, expected,
            "Decoded point cloud points do not match expected"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_mesh_buffer_len() {
        #[cfg(not(target_arch = "wasm32"))]
        let input = fs::read("assets/extracted_model/extracted_model_data.bin")
            .expect("Failed to read model file");

        let expect_len = debug_estimate_draco_buffer_len(&input);

        let mut config = MeshDecodeConfig::new(16744, 54663);
        config.add_attribute(3, AttributeDataType::Float32);
        config.add_attribute(2, AttributeDataType::Float32);

        let actual_len = config.estimate_buffer_size();
        println!("{actual_len}");

        assert_eq!(actual_len, expect_len);
    }

    async fn test_mesh(data: &[u8]) -> Vec<u8> {
        let mut config = MeshDecodeConfig::new(16744, 54663);
        config.add_attribute(3, AttributeDataType::Float32);
        config.add_attribute(2, AttributeDataType::Float32);

        let Some(buf) = decode_mesh(data, &config).await else {
            panic!("Mesh decode fail")
        };
        assert_eq!(buf.len(), config.estimate_buffer_size());

        buf
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_decode_mesh() {
        let input = fs::read("assets/extracted_model/extracted_model_data.bin")
            .expect("Failed to read model file");

        let out_buf = test_mesh(&input).await;

        fs::create_dir_all("assets/decode_model").ok();
        let path = "assets/decode_model/extracted_model_data.bin";
        fs::write(path, &out_buf).expect("Failed to write decoded mesh binary");
        println!("Wrote decoded mesh to {path}");
    }

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_config() {
        let mut config = MeshDecodeConfig::new(16744, 54663);
        config.add_attribute(3, AttributeDataType::Float32);
        config.add_attribute(2, AttributeDataType::Float32);

        assert_eq!(config.index_length(), 109326);

        let Some(attr_0) = config.get_attribute(0) else {
            panic!("fail to get attribute 0")
        };

        assert_eq!(attr_0.offset(), 109326);
        assert_eq!(attr_0.lenght(), 200928);

        let Some(attr_1) = config.get_attribute(1) else {
            panic!("fail to get attribute 0")
        };

        assert_eq!(attr_1.offset(), 310254);
        assert_eq!(attr_1.lenght(), 133952);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_decode_mesh_wasm() {
        use super::*;
        use wasm_bindgen::*;
        use wasm_bindgen_futures::spawn_local;
        use web_sys::console;

        async fn fetch_binary(url: &str) -> Result<Vec<u8>, JsValue> {
            use wasm_bindgen_futures::JsFuture;
            use web_sys::{Request, RequestInit, RequestMode, Response};

            let opts = RequestInit::new();
            opts.set_method("GET");
            opts.set_mode(RequestMode::Cors);

            let request = Request::new_with_str_and_init(url, &opts)?;
            let resp_value =
                JsFuture::from(web_sys::window().unwrap().fetch_with_request(&request)).await?;
            let resp: Response = resp_value.dyn_into().unwrap();
            if !resp.ok() {
                return Err(JsValue::from_str("Fetch failed"));
            }
            let buf = JsFuture::from(resp.array_buffer()?).await?;
            let u8_array = js_sys::Uint8Array::new(&buf);
            let mut body = vec![0; u8_array.length() as usize];
            u8_array.copy_to(&mut body[..]);
            Ok(body)
        }

        spawn_local(async {
            console::log_1(&"Starting wasm test...".into());

            match fetch_binary("assets/extracted_model/extracted_model_data.bin").await {
                Ok(data) => {
                    test_mesh(&data).await;
                }
                Err(e) => console::error_1(&format!("Fetch error: {:?}", e).into()),
            }
        });
    }
}
