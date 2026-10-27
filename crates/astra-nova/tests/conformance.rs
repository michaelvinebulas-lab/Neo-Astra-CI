//! Runs the backend-neutral conformance corpus against Nova.
//!
//! The corpus lives in `conformance/cases`. Each case is a self-contained
//! module with a `pub fn main() -> i32`; its `.expected` file holds the exact
//! stdout followed by a final `RETURN=<code>` line. See
//! `conformance/README.md`.
//!
//! Nova is the second independent implementation to run the corpus (JavaScript
//! is the first). Known gaps are listed explicitly instead of being skipped
//! silently: a gap that starts passing fails this test so the ledger cannot rot.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use astra_nova::{Host, NovaOptions, build_interpreter};

/// Cases Nova cannot run yet. Each entry must name the owning task.
///
/// When a case starts passing, remove it from this table; the test fails on
/// stale entries so gaps stay visible instead of becoming permanent skips.
const KNOWN_GAPS: &[(&str, &str)] = &[
    ("bytes_buffer", "Nova has no `bytes` builtin yet (NOVA-004)"),
    (
        "channels",
        "Nova has no Channel runtime; tracked by CONC-001",
    ),
    (
        "strings_utf8",
        "Nova implements only `str.len`; byte_at/char_at/slice/char_count/chars/bytes are missing (NOVA-004)",
    ),
];

#[derive(Clone)]
struct CaptureHost {
    stdout: Rc<RefCell<String>>,
    stderr: Rc<RefCell<String>>,
}

impl Host for CaptureHost {
    fn stdout(&mut self, text: &str) {
        self.stdout.borrow_mut().push_str(text);
    }

    fn stderr(&mut self, text: &str) {
        self.stderr.borrow_mut().push_str(text);
    }
}

fn cases_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases")
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_string()
}

fn render(stdout: &str, code: i128) -> String {
    let mut rendered = stdout.trim_end_matches('\n').to_string();
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered.push_str(&format!("RETURN={code}"));
    rendered
}

fn known_gap(name: &str) -> Option<&'static str> {
    KNOWN_GAPS
        .iter()
        .find(|(case, _)| *case == name)
        .map(|(_, reason)| *reason)
}

#[test]
fn nova_passes_the_shared_conformance_suite() {
    let directory = cases_directory();
    let mut cases = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ns"))
        .collect::<Vec<_>>();
    cases.sort();

    assert!(
        !cases.is_empty(),
        "no conformance cases found in {}",
        directory.display()
    );

    let mut executed = 0;
    let mut failures = Vec::new();

    for path in cases {
        let name = path.file_stem().map_or_else(
            || "<unknown>".to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let expected_path = path.with_extension("expected");
        let expected =
            normalize(&fs::read_to_string(&expected_path).unwrap_or_else(|error| {
                panic!("cannot read {}: {error}", expected_path.display())
            }));

        let stdout = Rc::new(RefCell::new(String::new()));
        let stderr = Rc::new(RefCell::new(String::new()));
        let host = CaptureHost {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        };

        let result = build_interpreter(&source, NovaOptions::default(), Box::new(host))
            .and_then(|mut interpreter| interpreter.run());
        let captured = stdout.borrow().clone();
        let errors = stderr.borrow().clone();

        if let Some(reason) = known_gap(&name) {
            if result.is_ok() {
                failures.push(format!(
                    "{name}: listed as a Nova gap but it now passes; remove it from KNOWN_GAPS ({reason})"
                ));
            }
            continue;
        }

        executed += 1;
        match result {
            Ok(value) => {
                let code = value.as_integer().unwrap_or(0);
                let actual = render(&captured, code);
                if actual != expected {
                    failures.push(format!(
                        "{name}: output mismatch\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
                    ));
                }
            }
            Err(error) => {
                failures.push(format!(
                    "{name}: Nova failed\n--- stdout ---\n{captured}\n--- stderr ---\n{errors}\n--- error ---\n{error:?}"
                ));
            }
        }
    }

    for (name, _) in KNOWN_GAPS {
        if !cases_directory().join(format!("{name}.ns")).exists() {
            failures.push(format!(
                "{name}: listed in KNOWN_GAPS but conformance/cases/{name}.ns does not exist"
            ));
        }
    }

    assert!(
        executed > 0,
        "no conformance case was executed; every case is in KNOWN_GAPS"
    );

    assert!(
        failures.is_empty(),
        "{} conformance problem(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
