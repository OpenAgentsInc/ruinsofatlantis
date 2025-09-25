//! Headless simulation harness CLI.
//! Usage: cargo run --bin sim_harness -- --scenario data/scenarios/example.yaml

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let scenario = args.iter().skip_while(|a| a.as_str() != "--scenario").nth(1);
    match scenario {
        Some(path) => {
            println!("[sim] loading scenario: {path}");
            // TODO: load via core::data scenario loaders and run sim
            println!("[sim] (stub) run complete.");
        }
        None => {
            eprintln!("usage: sim_harness --scenario <path>");
        }
    }
}

