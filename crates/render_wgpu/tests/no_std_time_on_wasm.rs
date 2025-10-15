// Guardrail test: ensure renderer passes avoid using std::time::Instant::now
// in files that are executed on wasm. This catches regressions that would
// trigger the "time not implemented on this platform" panic in browsers.

use std::fs;
use std::path::PathBuf;

fn file(p: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(p);
    fs::read_to_string(&path).expect("read source file")
}

#[test]
fn passes_graph_uses_time_alias_not_std_time() {
    let s = file("gfx/renderer/passes_graph.rs");
    assert!(
        !s.contains("std::time::Instant::now("),
        "std::time::Instant::now present in passes_graph.rs; use cfg alias to web_time::Instant on wasm"
    );
}

#[test]
fn init_uses_time_alias_not_std_time() {
    let s = file("gfx/renderer/init.rs");
    assert!(
        !s.contains("std::time::Instant::now("),
        "std::time::Instant::now present in init.rs; use cfg alias to web_time::Instant on wasm"
    );
}

#[test]
fn mod_uses_time_alias_not_std_time() {
    let s = file("gfx/mod.rs");
    assert!(
        !s.contains("std::time::Instant::now("),
        "std::time::Instant::now present in mod.rs; use cfg alias to web_time::Instant on wasm"
    );
}
