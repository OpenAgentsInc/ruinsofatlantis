use js_sys::{Promise, Uint8Array};
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::window;

use crate::{AttributeDataType, AttributeValues, MeshDecodeConfig};

use web_sys::console;

thread_local! {
    static DRACO_DECODE_FUNC_MODULE: RefCell<Option<JsValue>> = RefCell::new(None);
}

async fn get_js_module() -> Result<JsValue, JsValue> {
    if let Some(module) = DRACO_DECODE_FUNC_MODULE.with(|m| m.borrow().clone()) {
        return Ok(module);
    }

    let js_code = include_str!("../javascript/index.es.js");
    let escaped = js_code.replace("\\", "\\\\").replace("`", "\\`");

    let setup_code = format!(
        r#"
        (function() {{
            const code = `{escaped}`;
            const blob = new Blob([code], {{ type: "application/javascript" }});
            const url = URL.createObjectURL(blob);
            return import(url).then(mod => {{
                URL.revokeObjectURL(url);
                return mod;
            }});
        }})()
    "#
    );

    // Use eval to run the wrapper and return a promise of the module
    let js_module = js_sys::eval(&setup_code)?;
    let module_promise: Promise = js_module.dyn_into()?;
    let module = JsFuture::from(module_promise).await?;

    DRACO_DECODE_FUNC_MODULE.with(|m| m.replace(Some(module.clone())));

    Ok(module)
}

async fn decode_draco_mesh_from_embedded_js(
    data: &js_sys::Uint8Array,
    byte_length: usize,
) -> Result<js_sys::Uint8Array, JsValue> {
    let module = get_js_module().await?;

    // Call the decode function from the module
    let decode_fn = js_sys::Reflect::get(&module, &JsValue::from_str("decodeDracoMeshInWorker"))?
        .dyn_into::<js_sys::Function>()?;

    let this = JsValue::NULL;
    let result = decode_fn.call2(&this, data, &JsValue::from(byte_length))?;
    let decode_promise: Promise = result.dyn_into()?;
    let out_buf = JsFuture::from(decode_promise).await?;
    Ok(out_buf.dyn_into::<Uint8Array>()?)
}

pub async fn decode_mesh_wasm_worker(data: &[u8], config: &MeshDecodeConfig) -> Option<Vec<u8>> {
    let js_array = Uint8Array::from(data);
    let estimate_buffer_size = config.estimate_buffer_size();

    match decode_draco_mesh_from_embedded_js(&js_array, estimate_buffer_size).await {
        Ok(decoded) => Some(decoded.to_vec()),
        Err(err) => {
            web_sys::console::error_1(&err);
            None
        }
    }
}
