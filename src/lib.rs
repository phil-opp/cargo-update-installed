use std::collections::{BTreeMap, HashMap};
use std::io::{stderr, Write};
use std::process::{Command, ExitStatus};

extern crate tempdir;
extern crate toml;

pub fn installed_crates() -> Result<BTreeMap<String, Crate>, String> {
    let mut cargo_list_installed = Command::new("cargo");
    cargo_list_installed.arg("install");
    cargo_list_installed.arg("--list");
    let installed_output = cargo_list_installed
        .output()
        .map_err(|e| format!("I/O Error: {}", e))?;
    let installed =
        String::from_utf8(installed_output.stdout).map_err(|e| format!("UTF-8 Error: {}", e))?;

    let mut crates: BTreeMap<String, Crate> = BTreeMap::new();
    for line in installed.lines() {
        let _crate = Crate::parse_list_output(line).map_err(|e| format!("Error: {:?}", e))?;
        if let Some(_crate) = _crate {
            if let Some(c) = crates.get(&_crate.name) {
                // only consider latest version
                // (It is possible to have two different versions of the same crate installed,
                // for example when an old version contained an executable that is no longer
                // present in the newer version.)
                if c.version > _crate.version {
                    continue;
                }
            }
            crates.insert(_crate.name.clone(), _crate);
        }
    }
    Ok(crates)
}

pub fn get_latest_versions(
    required_crates: &HashMap<String, Crate>,
) -> Result<HashMap<String, String>, String> {
    use std::fs;
    use tempdir::TempDir;

    fn dependency_string(required_crates: &HashMap<String, Crate>) -> String {
        let mut string = String::new();
        for c in required_crates.values() {
            match c.kind {
                CrateKind::CratesIo => {
                    string.push_str(&format!(r#"{} = "{}"{}"#, c.name, c.version, '\n'));
                }
            }
        }
        string
    }

    fn create_dummy_crate(required_crates: &HashMap<String, Crate>) -> Result<TempDir, String> {
        let tmpdir = TempDir::new("cargo-update-installed")
            .map_err(|e| format!("I/O Error while creating temporary directory: {}", e))?;
        let cargo_toml_path = tmpdir.path().join("Cargo.toml");
        let src_dir_path = tmpdir.path().join("src");
        let lib_rs_path = src_dir_path.join("lib.rs");

        let cargo_toml_content = format!(
            r#"[package]
name = "cargo-update-installed-dummy"
version = "0.1.0"
authors = [""]

[dependencies]
{}
"#,
            dependency_string(required_crates)
        );

        fs::create_dir(src_dir_path)
            .map_err(|e| format!("I/O Error while creating src dir in temp dir: {}", e))?;
        fs::write(cargo_toml_path, cargo_toml_content)
            .map_err(|e| format!("I/O Error while writing dummy Cargo.toml: {}", e))?;
        fs::write(lib_rs_path, "")
            .map_err(|e| format!("I/O Error while writing dummy lib.rs: {}", e))?;
        Ok(tmpdir)
    }

    fn run_cargo_update(tmpdir: &TempDir) -> Result<ExitStatus, String> {
        let mut cargo_update_command = Command::new("cargo");
        cargo_update_command.arg("update");
        cargo_update_command.arg("--manifest-path");
        cargo_update_command.arg(tmpdir.path().join("Cargo.toml"));
        cargo_update_command
            .status()
            .map_err(|e| format!("I/O Error while running `cargo update`: {}", e))
    }

    fn parse_cargo_lock(
        tmpdir: &TempDir,
        required_crates: &HashMap<String, Crate>,
    ) -> Result<HashMap<String, String>, String> {
        use std::fs;
        use toml::Value;

        let cargo_lock_path = tmpdir.path().join("Cargo.lock");
        let cargo_lock = fs::read_to_string(cargo_lock_path)
            .map_err(|e| format!("I/O Error while reading dummy Cargo.lock: {}", e))?;

        let root_value: Value = cargo_lock
            .parse()
            .map_err(|e| format!("Error while parsing dummy Cargo.lock: {}", e))?;
        let packages = root_value
            .get("package")
            .and_then(|v| v.as_array())
            .ok_or("Error: package array not found in dummy Cargo.lock")?;

        let mut latest_versions = HashMap::new();
        for crate_name in required_crates.keys() {
            let package = packages
                .iter()
                .find(|p| p.get("name").and_then(|v| v.as_str()) == Some(crate_name))
                .ok_or(format!(
                    "Error: package {} not found in dummy Cargo.lock",
                    crate_name
                ))?;
            let version = package
                .get("version")
                .and_then(|v| v.as_str())
                .ok_or(format!(
                    "Error: package {} has no version number in dummy Cargo.lock",
                    crate_name
                ))?;
            if latest_versions
                .insert(crate_name.clone(), String::from(version))
                .is_some()
            {
                writeln!(stderr(), "Warning: package {} is present multiple times in dummy Cargo.lock. Choosing version {}.", crate_name, version);
            }
        }
        Ok(latest_versions)
    }

    let tmpdir = create_dummy_crate(required_crates)?;
    if !run_cargo_update(&tmpdir)?.success() {
        return Err("Error: `cargo update` failed".into());
    }
    parse_cargo_lock(&tmpdir, required_crates)
}

pub fn install_update(name: &str, version: &str) -> Result<ExitStatus, String> {
    let mut cargo_install_command = Command::new("cargo");
    cargo_install_command.arg("install");
    cargo_install_command.arg("--force");
    cargo_install_command.arg(name);
    cargo_install_command.arg("--version");
    cargo_install_command.arg(version);
    cargo_install_command
        .status()
        .map_err(|e| format!("I/O Error while running `cargo install`: {}", e))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crate {
    pub name: String,
    pub version: String,
    pub kind: CrateKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrateKind {
    CratesIo,
    /*
    Git {
        url: String,
        branch: Option<String>,
    },
    Local,
*/
}

impl Crate {
    /// Parses a line from `cargo install --list`.
    pub fn parse_list_output(line: &str) -> Result<Option<Crate>, error::ParseListOutputError> {
        use error::ParseListOutputError;

        if line.starts_with(" ") {
            return Ok(None);
        }

        let mut parts = line.split(" ");
        let name = parts.next().ok_or(ParseListOutputError)?;

        let version = parts.next().ok_or(ParseListOutputError)?;
        if !version.starts_with("v") {
            return Err(ParseListOutputError);
        }
        let version = version.trim_start_matches("v");

        if version.ends_with(":") {
            // crates.io dependency
            let version = version.trim_end_matches(":");
            Ok(Some(Crate {
                name: name.into(),
                version: version.into(),
                kind: CrateKind::CratesIo,
            }))
        } else {
            let dependency_path = parts.next().ok_or(ParseListOutputError)?;
            if !dependency_path.starts_with("(") || !dependency_path.ends_with("):") {
                return Err(ParseListOutputError);
            }
            let dependency_path = dependency_path
                .trim_start_matches("(")
                .trim_end_matches("):");

            if dependency_path.starts_with("http") {
                // git dependency
                writeln!(
                    stderr(),
                    "Warning: Git binaries are not supported. Ignoring `{}`.",
                    name
                );
                Ok(None)
            } else {
                // local dependency
                writeln!(
                    stderr(),
                    "Warning: Local binaries are not supported. Ignoring `{}`.",
                    name
                );
                Ok(None)
            }
        }
    }
}

pub mod error {
    #[derive(Debug)]
    pub struct ParseListOutputError;
}
