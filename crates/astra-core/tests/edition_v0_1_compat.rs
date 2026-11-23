//! Phase 1 gate: default and explicit v0.1 behavior is unchanged, and the
//! 0.2 edition still accepts the v0.1 surface until the grammar phases land.

use astra_core::edition::Edition;
use astra_core::formatter::{FormatOptions, format_source, format_source_with_options};
use astra_core::lexer::{LexOptions, lex, lex_with_options};
use astra_core::parser::{ParseOptions, parse, parse_with_options};

const SOURCE: &str = r#"module demo;

function main() -> i32 {
  var x = 1;
  return x;
}
"#;

#[test]
fn default_lexing_matches_explicit_v0_1() {
    assert_eq!(
        lex(SOURCE),
        lex_with_options(
            SOURCE,
            LexOptions {
                edition: Edition::V0_1
            }
        )
    );
}

#[test]
fn default_parsing_matches_explicit_v0_1() {
    assert_eq!(
        parse(SOURCE),
        parse_with_options(
            SOURCE,
            ParseOptions {
                edition: Edition::V0_1
            }
        )
    );
}

#[test]
fn default_formatting_is_byte_identical_to_explicit_v0_1() {
    let default = format_source(SOURCE).expect("default format");
    let explicit = format_source_with_options(
        SOURCE,
        FormatOptions {
            edition: Edition::V0_1,
        },
    )
    .expect("explicit v0.1 format");
    assert_eq!(default, explicit);
}

#[test]
fn v0_2_edition_still_parses_the_v0_1_surface() {
    let v0_1 = parse_with_options(
        SOURCE,
        ParseOptions {
            edition: Edition::V0_1,
        },
    )
    .expect("v0.1 parse");
    let v0_2 = parse_with_options(
        SOURCE,
        ParseOptions {
            edition: Edition::V0_2,
        },
    )
    .expect("v0.2 parse of v0.1 surface");
    assert_eq!(v0_1, v0_2);
}
