//! Native `nopm` launcher for the standalone and npm distributions.
//!
//! The Bun-built `astra` binary also dispatches `astra nopm`, but Windows does
//! not preserve a hardlinked executable name in `process.argv[0]`. This tiny
//! launcher forwards the command explicitly so npm's `nopm` bin shim works on
//! every host without duplicating the embedded runtime.

use std::process::{Command, exit};

fn main() {
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("nopm: cannot resolve the launcher path: {error}");
            exit(1);
        }
    };
    let directory = match executable.parent() {
        Some(directory) => directory,
        None => {
            eprintln!("nopm: the launcher has no parent directory");
            exit(1);
        }
    };
    let astra = [directory.join("astra.exe"), directory.join("astra")]
        .into_iter()
        .find(|candidate| candidate.is_file());
    let Some(astra) = astra else {
        eprintln!("nopm: cannot find astra beside the launcher");
        exit(1);
    };

    let status = Command::new(astra)
        .arg("nopm")
        .args(std::env::args_os().skip(1))
        .status();
    match status {
        Ok(status) => exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("nopm: cannot start astra: {error}");
            exit(1);
        }
    }
}
