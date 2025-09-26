#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;
#[cfg(target_arch = "wasm32")]
use web_sys::console;

#[cfg(target_arch = "wasm32")]
async fn fetch_binary(url: &str) -> Result<Vec<u8>, JsValue> {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    let mut opts = RequestInit::new();
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

fn main() {
    #[cfg(target_arch = "wasm32")]
    spawn_local(async {
        use draco_decoder::{AttributeDataType, MeshDecodeConfig};

        console::log_1(&"Starting wasm test...".into());

        let mut config = MeshDecodeConfig::new(16744, 54663);
        config.add_attribute(3, AttributeDataType::Float32);
        config.add_attribute(2, AttributeDataType::Float32);

        match fetch_binary("assets/extracted_model/extracted_model_data.bin").await {
            Ok(bin) => {
                use draco_decoder::decode_mesh;
                let perf = web_sys::window().unwrap().performance().unwrap();

                let start = perf.now();

                match decode_mesh(&bin, &config).await {
                    Some(mesh) => {
                        let end = perf.now();
                        console::log_1(&format!("Decode time: {:.2} ms", end - start).into());
                        use wasm_bindgen::JsCast;
                        use web_sys::{Blob, HtmlElement, Url};

                        // 创建 Blob
                        let array = js_sys::Uint8Array::from(&mesh[..]);
                        let parts = js_sys::Array::new();
                        parts.push(&array.buffer());
                        let blob = Blob::new_with_u8_array_sequence(&parts).unwrap();

                        // 生成下载链接
                        let url = Url::create_object_url_with_blob(&blob).unwrap();
                        let window = web_sys::window().unwrap();
                        let document = window.document().unwrap();
                        let a = document.create_element("a").unwrap();
                        a.set_attribute("href", &url).unwrap();
                        a.set_attribute("download", "extracted_model_data.bin")
                            .unwrap();
                        let a_elem: HtmlElement = a.dyn_into().unwrap();
                        a_elem.click();
                        Url::revoke_object_url(&url).unwrap();
                    }
                    None => console::error_1(&format!("Decode Fail").into()),
                }
            }
            Err(e) => console::error_1(&format!("Fetch error: {:?}", e).into()),
        }
    });
}
