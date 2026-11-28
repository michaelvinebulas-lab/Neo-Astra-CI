//! Migration corpus: v0.1 sources must migrate to canonical v0.2 spellings.

use std::fs;
use std::path::PathBuf;

use astra_core::canonical::check_canonical;
use astra_core::edition::Edition;
use astra_core::formatter::{FormatOptions, format_source_with_options};
use astra_core::migrate::{MigrateOptions, migrate_source};

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/v0.2/migrate")
}

#[test]
fn migration_corpus_becomes_canonical_v0_2() {
    for entry in fs::read_dir(corpus_path()).expect("migrate directory") {
        let path = entry.expect("entry").path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".input.ns") {
            continue;
        }
        let input = fs::read_to_string(&path).expect("read input");

        let migrated = migrate_source(&input, MigrateOptions::default())
            .unwrap_or_else(|diagnostics| panic!("{name}: {diagnostics:?}"));
        let formatted = format_source_with_options(
            &migrated.source,
            FormatOptions {
                edition: Edition::V0_2,
            },
        )
        .unwrap_or_else(|diagnostics| panic!("{name}: {diagnostics:?}"));

        let lint = check_canonical(&formatted, Edition::V0_2);
        assert!(lint.is_empty(), "{name}: {lint:?}");

        for deprecated in [
            "public ",
            "function ",
            "var ",
            "mutable ",
            "constant ",
            "typedef ",
            "record ",
            "{@mode:",
            "{@memory:",
            "spawn ||",
            "!)",
        ] {
            assert!(
                !formatted.contains(deprecated),
                "{name}: still contains '{deprecated}'\n{formatted}"
            );
        }
        for canonical in [
            "pub fn ", "fn ", "let ", "let mut ", "const ", "type ", "struct ", "@mode(",
        ] {
            if input.contains(canonical.trim()) || formatted.contains(canonical) {
                assert!(
                    formatted.contains(canonical),
                    "{name}: missing '{canonical}'\n{formatted}"
                );
            }
        }
    }
}

#[test]
fn migration_rewrites_error_forms_and_spawn() {
    let input = fs::read_to_string(corpus_path().join("errors_and_spawn.input.ns"))
        .expect("read errors_and_spawn input");
    let migrated = migrate_source(&input, MigrateOptions::default())
        .expect("migrate errors_and_spawn")
        .source;
    assert!(
        migrated.contains("fail NotFound(\"missing\")"),
        "{migrated}"
    );
    assert!(migrated.contains("parse()?"), "{migrated}");
    assert!(migrated.contains("spawn { return 1 }"), "{migrated}");
}
