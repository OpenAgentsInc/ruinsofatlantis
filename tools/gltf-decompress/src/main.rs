use std::{env, ffi::OsStr, path::PathBuf, process::Command};

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(input) = args.next() else {
        eprintln!("Missing <input.gltf>");
        std::process::exit(2)
    };
    let Some(output) = args.next() else {
        eprintln!("Missing <output.gltf>");
        std::process::exit(2)
    };

    let input = PathBuf::from(input);
    let output = PathBuf::from(output);

    let candidates: Vec<Vec<&OsStr>> = vec![
        vec![
            OsStr::new("@gltf-transform/cli"),
            OsStr::new("draco"),
            OsStr::new("-d"),
            input.as_os_str(),
            output.as_os_str(),
        ],
        vec![
            OsStr::new("gltf-transform"),
            OsStr::new("draco"),
            OsStr::new("-d"),
            input.as_os_str(),
            output.as_os_str(),
        ],
        vec![
            OsStr::new("@gltf-transform/cli"),
            OsStr::new("decompress"),
            input.as_os_str(),
            output.as_os_str(),
        ],
        vec![
            OsStr::new("gltf-transform"),
            OsStr::new("decompress"),
            input.as_os_str(),
            output.as_os_str(),
        ],
    ];

    // Try npx
    if let Ok(npx) = which::which("npx") {
        for args in &candidates {
            if run(npx.as_os_str(), Some(OsStr::new("-y")), args) {
                return;
            }
        }
    }
    // Try global
    for args in &candidates {
        if run(args[0], None, &args[1..]) {
            return;
        }
    }

    eprintln!(
        "Failed to run gltf-transform via npx or globally. Please install Node and try:\n  npx -y @gltf-transform/cli draco -d {} {}",
        input.display(),
        output.display()
    );
    std::process::exit(1);
}

fn run(cmd: &std::ffi::OsStr, first: Option<&OsStr>, rest: &[&OsStr]) -> bool {
    let mut c = Command::new(cmd);
    if let Some(f) = first {
        c.arg(f);
    }
    let status = c.args(rest).status();
    match status {
        Ok(s) if s.success() => {
            println!("OK");
            true
        }
        Ok(s) => {
            eprintln!("{} exited with status {s}", cmd.to_string_lossy());
            false
        }
        Err(e) => {
            eprintln!("{} failed: {e}", cmd.to_string_lossy());
            false
        }
    }
}
