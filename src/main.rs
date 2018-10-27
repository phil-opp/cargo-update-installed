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
    use std::collections::HashMap;

    let installed_crates = installed_crates()?;
    let mut required_crates = HashMap::new();
    for c in installed_crates.values() {
        if required_crates.contains_key(&c.name) {
            println!("Ignoring duplicate installed crate: {:?}", c);
            continue;
        }

        let mut required_crate = c.clone();
        // require latest version with no constraints
        required_crate.version = "*".into();
        if required_crates
            .insert(c.name.clone(), required_crate)
            .is_some()
        {
            unreachable!("duplicate key");
        }
    }

    let latest_versions = get_latest_versions(&required_crates)?;

    let mut updates = Vec::new();
    for c in installed_crates.values() {
        let latest_version = latest_versions
            .get(&c.name)
            .ok_or(format!("Error: No latest version found for {}", c.name))?;
        if &c.version != latest_version {
            updates.push((c, latest_version));
        } else {
            println!("Up to date: {} {}", c.name, c.version);
        }
    }
    updates.sort_unstable_by_key(|(c, _)| &c.name);

    for (_crate, latest_version) in &updates {
        println!(
            "\nUpdating {} from {} to {}",
            _crate.name, _crate.version, latest_version
        );
        if !install_update(&_crate.name, latest_version)?.success() {
            return Err("Error: `cargo install` failed".into());
        }
    }

    println!("\nAll packages up to date.");

    Ok(())
}
