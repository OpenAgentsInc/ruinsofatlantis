//! Headless simulation harness CLI.
//! Usage: cargo run --bin sim_harness -- --scenario data/scenarios/example.yaml

use std::env;
use std::path::Path;
use ruinsofatlantis::core::data::{scenario, loader};
use ruinsofatlantis::sim::runner;

fn main() {
    let args: Vec<String> = env::args().collect();
    let scenario = args.iter().skip_while(|a| a.as_str() != "--scenario").nth(1);
    let result_only = args.iter().any(|a| a == "--result-only" || a == "-q");
    match scenario {
        Some(path) => {
            println!("[sim] loading scenario: {path}");
            let p = Path::new(path);
            match scenario::load_yaml(p) {
                Ok(scn) => {
                    println!("[sim] name={} tick_ms={} seed={:?} underwater={}", scn.name, scn.tick_ms, scn.seed, scn.underwater);
                    println!("[sim] actors: {}", scn.actors.len());
                    // Showcase loading Fire Bolt spec (if present)
                    if let Ok(fb) = loader::load_spell_spec("spells/fire_bolt.json") {
                        println!("[sim] loaded spell: {} (lvl {})", fb.name, fb.level);
                        if let Some(dmg) = fb.damage { println!("[sim] damage.type={}", dmg.damage_type); }
                    }
                    runner::run_scenario(&scn, result_only);
                }
                Err(e) => eprintln!("[sim] failed to load scenario: {e}"),
            }
        }
        None => {
            eprintln!("usage: sim_harness --scenario <path>");
        }
    }
}
