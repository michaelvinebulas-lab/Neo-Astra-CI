//! Phase 7 slice 4: edition-0.2 `char` and scalar literals.

use astra_core::checker::{CheckOptions, check_source};
use astra_core::edition::Edition;
use astra_core::formatter::{FormatOptions, format_source_with_options};

fn check_v02(source: &str) -> Vec<String> {
    match check_source(source, CheckOptions::default().with_edition(Edition::V0_2)) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    }
}

fn format_v02(source: &str) -> String {
    format_source_with_options(
        source,
        FormatOptions {
            edition: Edition::V0_2,
        },
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"))
}

#[test]
fn char_literals_and_type_typecheck() {
    let diagnostics = check_v02(
        r#"
module app.main

fn label(value: char) -> str {
    return match value {
        'a' => "first",
        _ => "other",
    }
}

pub fn main() -> i32 {
    let letter = 'a'
    let newline = '\n'
    let emoji = '\u{1F600}'
    let quote = '\''
    let optional: Option<char> = Some('b')
    println(letter)
    println(label(letter))
    println(optional.unwrap_or('z'))
    if letter == 'a' {
        println(newline)
    }
    if emoji != 'x' {
        println(quote)
    }
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn char_is_not_numeric_or_ordered() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    if 'a' < 'b' {
        return 1
    }
    return 0
}
"#,
    );
    assert!(
        diagnostics.contains(&"NS4006".to_owned()),
        "{diagnostics:?}"
    );
}

#[test]
fn edition_0_1_rejects_character_literals() {
    let diagnostics = match check_source("let x = 'a'\n", CheckOptions::default()) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    };
    assert!(
        diagnostics
            .iter()
            .any(|code| code == "NS1005" || code == "NS2001"),
        "{diagnostics:?}"
    );
}

#[test]
fn formatter_round_trips_character_literals() {
    let formatted = format_v02(
        r#"
module app.main

fn value() -> char {
    return '\u{1F600}'
}
"#,
    );
    assert!(formatted.contains("return '😀'"), "{formatted}");
    assert_eq!(format_v02(&formatted), formatted);
}
