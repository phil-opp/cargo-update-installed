use std::collections::BTreeMap;
use std::io::{stderr, Write};
use std::process::{Command, ExitStatus};

extern crate tempfile;
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

pub fn install_update(name: &str) -> Result<ExitStatus, String> {
    let mut cargo_install_command = Command::new("cargo");
    cargo_install_command.arg("install");
    cargo_install_command.arg(name);
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
                version: version.parse().map_err(|_| ParseListOutputError)?,
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
