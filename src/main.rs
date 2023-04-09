use std::io::{stderr, Write};

extern crate cargo_update_installed;

use cargo_update_installed::*;

fn main() {
    match run() {
        Ok(()) => {}
        Err(err) => {
            writeln!(stderr(), "Error: {}", err);
        }
    };
}

fn run() -> Result<(), String> {
    let installed_crates = installed_crates()?;

    for c in installed_crates.keys() {
        println!("Updating `{c}`");
        if !install_update(c)?.success() {
            return Err("Error: `cargo install` failed".into());
        }
    }

    Ok(())
}
