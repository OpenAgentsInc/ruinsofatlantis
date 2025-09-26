use std::path::Path;
use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() == "wasm32" {
        println!("cargo:warning=Skipping build.rs on wasm32 target");
        return;
    }

    // Step 1: Build Draco with CMake
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let draco_dir = "third_party/draco";
    let draco_build = format!("{draco_dir}/build");
    let draco_install = format!("{draco_build}/install");

    if !Path::new(&draco_build).exists() {
        std::fs::create_dir_all(&draco_build).unwrap();
    }

    let status = Command::new("cmake")
        .args([
            "..",
            "-G",
            "Unix Makefiles",
            "-DBUILD_SHARED_LIBS=OFF",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DDRACO_TESTS=OFF",
            &format!("-DCMAKE_INSTALL_PREFIX={}", "install"),
        ])
        .current_dir(&draco_build)
        .status()
        .expect("Failed to run CMake");
    assert!(status.success(), "CMake configuration failed");

    let status = Command::new("cmake")
        .args(["--build", "."])
        .current_dir(&draco_build)
        .status()
        .expect("Failed to build Draco");
    assert!(status.success(), "Draco build failed");

    let status = Command::new("cmake")
        .args(["--install", "."])
        .current_dir(&draco_build)
        .status()
        .expect("Failed to install Draco");
    assert!(status.success(), "Draco install failed");

    let mut build = cxx_build::bridge("src/ffi.rs");
    // Resolve include paths relative to the crate dir to ensure correctness
    // when used as a path dependency.
    build
        .file("cpp/decoder_api.cc")
        .include(format!("{}/{}", crate_dir, "include"))
        .include(format!("{}/{}", crate_dir, "third_party/draco/src"))
        .include(format!("{}/{}", crate_dir, "third_party/draco/build"))
        .include(format!("{}/{}/include", crate_dir, draco_install))
        .flag_if_supported("-std=c++17")
        // Silence noisy third-party warnings in Draco headers
        .warnings(false)
        .flag_if_supported("-w");

    // Only set a macOS deployment target when building for macOS.
    // Passing this flag on Linux/Windows causes compiler errors.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build.flag("-mmacosx-version-min=15.5");
    }

    build.compile("decoder_api");

    let draco_lib_dir = Path::new(&crate_dir).join(draco_install).join("lib");
    println!("cargo:rustc-link-search=native={}", draco_lib_dir.display());
    println!("cargo:rustc-link-lib=static=draco");

    println!("cargo:rerun-if-changed=cpp/decoder_api.cc");
    println!("cargo:rerun-if-changed=include/decoder_api.h");
}
