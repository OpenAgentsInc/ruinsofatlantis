// A minimal Bevy app that prints "Hello, Bevy!" once at startup
// and opens a window using the default plugins. This follows the
// Bevy Quick Start guide's basic app layout.
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, hello_world)
        .run();
}

fn hello_world() {
    println!("Hello, Bevy!");
}
