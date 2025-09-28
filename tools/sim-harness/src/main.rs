use data_runtime::{loader, scenario};
use sim_core::sim::runner;
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: sim-harness <scenario.json>");
        std::process::exit(2);
    }
    let path = &args[1];
    let txt = loader::read_json(Path::new(path)).expect("read scenario json");
    let scen: scenario::Scenario = serde_json::from_str(&txt).expect("parse scenario json");
    let mut state = runner::SimState::from_scenario(&scen).expect("build state");
    // Run a fixed number of steps
    for _ in 0..1000 {
        runner::step(&mut state, 16);
    }
    println!("ok");
}
