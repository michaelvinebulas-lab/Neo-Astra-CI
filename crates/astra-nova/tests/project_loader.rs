//! Filesystem tests for the NOVA-003 project loader.
//!
//! The deterministic graph/runtime cases live in `project.rs`; these tests cover
//! canonical paths, root containment, I/O failures, and the public API.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use astra_core::mode::Mode;
use astra_nova::{
    BufferHost, ModuleId, NovaOptions, ProjectError, ProjectOptions, Value,
    build_project_interpreter, load_project,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "astra-nova-project-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("cannot create temp project");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.path.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("cannot create test subdirectory");
        }
        fs::write(&path, source).expect("cannot write test source");
        path
    }

    fn options(&self, entry: &str) -> ProjectOptions {
        ProjectOptions {
            root: self.path.clone(),
            entry: self.path.join(entry),
            mode: Mode::Core,
            infer_mode: true,
            allow_root_escape: false,
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn codes(error: &ProjectError) -> Vec<&str> {
    error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.diagnostic.code.as_str())
        .collect()
}

#[test]
fn loads_a_nested_project_from_disk() {
    let dir = TempDir::new();
    dir.write(
        "src/main.ns",
        "import { add } from \"../shared/math.ns\"\nfn main() -> i32 { return add(40) }",
    );
    dir.write(
        "shared/math.ns",
        "pub const BASE: i32 = 2\npub fn add(n: i32) -> i32 { return n + BASE }",
    );

    let project = load_project(dir.options("src/main.ns")).expect("project should load");
    assert_eq!(project.entry, ModuleId::parse("src/main.ns").unwrap());
    assert!(
        project
            .modules
            .contains_key(&ModuleId::parse("shared/math.ns").unwrap())
    );
    assert!(
        project.modules[&ModuleId::parse("src/main.ns").unwrap()]
            .text
            .contains("import")
    );

    let mut interpreter =
        build_project_interpreter(project, NovaOptions::default(), Box::new(BufferHost::new()))
            .expect("project should build");
    assert_eq!(
        interpreter.run().expect("project should run"),
        Value::Integer(astra_nova::Integer::new(42, astra_core::types::Type::I32))
    );
}

#[test]
fn rejects_an_entry_outside_the_project_root() {
    let dir = TempDir::new();
    dir.write("entry.ns", "fn main() {}");
    let root = dir.path().join("project");
    fs::create_dir_all(&root).expect("cannot create project root");

    let error = load_project(ProjectOptions {
        root,
        entry: dir.path().join("entry.ns"),
        mode: Mode::Core,
        infer_mode: true,
        allow_root_escape: false,
    })
    .expect_err("entry outside root should fail");
    assert_eq!(codes(&error), vec!["NS9008"]);
}

#[test]
fn rejects_missing_entries_as_io_errors() {
    let dir = TempDir::new();
    let error = load_project(dir.options("missing.ns")).expect_err("missing entry should fail");
    assert_eq!(codes(&error), vec!["NS9008"]);
}

#[test]
fn rejects_case_mismatched_imports_portably() {
    let dir = TempDir::new();
    dir.write(
        "main.ns",
        "import { value } from \"./Math.ns\"\nfn main() -> i32 { return value() }",
    );
    dir.write("math.ns", "pub fn value() -> i32 { return 42 }");

    let error = load_project(dir.options("main.ns"))
        .expect_err("case-mismatched import should fail on every platform");
    assert_eq!(codes(&error), vec!["NS3012"]);
}

#[test]
fn rejects_non_utf8_modules() {
    let dir = TempDir::new();
    let path = dir.path().join("main.ns");
    fs::write(&path, [0xFF, 0xFE, 0x00]).expect("cannot write invalid UTF-8");

    let error = load_project(dir.options("main.ns")).expect_err("invalid UTF-8 should fail");
    assert_eq!(codes(&error), vec!["NS9007"]);
}

#[test]
fn rejects_missing_local_modules_with_ns3012() {
    let dir = TempDir::new();
    dir.write(
        "main.ns",
        "import { value } from \"./missing.ns\"\nfn main() {}",
    );

    let error = load_project(dir.options("main.ns")).expect_err("missing module should fail");
    assert_eq!(codes(&error), vec!["NS3012"]);
}
