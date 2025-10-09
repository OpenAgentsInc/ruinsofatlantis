#[cfg(target_arch = "wasm32")]
pub fn is_enabled() -> bool {
    if let Some(win) = web_sys::window() {
        let q = win.location().search().ok().unwrap_or_default();
        let path = win.location().pathname().ok().unwrap_or_default();
        return q.contains("cc=1") || path.contains("/cc-demo");
    }
    false
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_enabled() -> bool {
    std::env::var("RA_CC_DEMO")
        .map(|v| v == "1")
        .unwrap_or(false)
}
