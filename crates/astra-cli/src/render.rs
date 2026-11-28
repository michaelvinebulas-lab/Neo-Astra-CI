//! Human-readable diagnostic rendering.
//!
//! Every CLI path renders diagnostics through this module, so `check <file>`,
//! `check <dir>`, `build`, `ast`, `explain`, and `fmt` produce the same block
//! for the same diagnostic. The shape follows rustc: a severity header, an
//! optional source location, an underlined snippet, then the hint.

use crate::ReportedDiagnostic;

/// Builds the underlined snippet for a span that points into `text`.
///
/// `line` is the 1-based line of `start`; the gutter width follows its decimal
/// length so multi-digit line numbers stay aligned. Returns `None` when the
/// span is outside the source.
pub fn snippet(text: &str, start: usize, end: usize, line: usize) -> Option<String> {
    if start > text.len() {
        return None;
    }
    let end = end.min(text.len()).max(start);
    let line_start = text[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[start..]
        .find('\n')
        .map_or(text.len(), |index| start + index);
    let source_line = text[line_start..line_end].trim_end_matches('\r');
    let column = text[line_start..start].chars().count();
    let carets = text[start..end.min(line_end)].chars().count().max(1);
    let gutter = " ".repeat(line.to_string().len());
    Some(format!(
        "{gutter} |\n{line:>width$} | {source_line}\n{gutter} | {padding}{carets}",
        gutter = gutter,
        line = line,
        width = gutter.len(),
        source_line = source_line,
        padding = " ".repeat(column),
        carets = "^".repeat(carets),
    ))
}

/// Renders one diagnostic as a block without a trailing newline.
pub fn render(diagnostic: &ReportedDiagnostic) -> String {
    let gutter = " ".repeat(diagnostic.line.map_or(1, |line| line.to_string().len()));
    let mut out = format!(
        "{}[{}]: {}",
        diagnostic.severity, diagnostic.code, diagnostic.message
    );
    if let Some(path) = diagnostic.path.as_deref() {
        out.push('\n');
        out.push_str(&gutter);
        out.push_str("--> ");
        out.push_str(path);
        if let Some(line) = diagnostic.line {
            out.push_str(&format!(":{}:{}", line, diagnostic.column.unwrap_or(1)));
        }
    }
    if let Some(snippet) = diagnostic.snippet.as_deref() {
        out.push('\n');
        out.push_str(snippet);
    }
    if let Some(hint) = diagnostic.hint.as_deref() {
        if diagnostic.snippet.is_some() {
            out.push('\n');
            out.push_str(&gutter);
            out.push_str(" |");
        }
        out.push('\n');
        out.push_str(&gutter);
        out.push_str(" = help: ");
        out.push_str(hint);
    }
    for related in &diagnostic.related {
        out.push_str("\nnote: ");
        out.push_str(&related.message);
        out.push('\n');
        out.push_str(&gutter);
        out.push_str("--> ");
        out.push_str(&related.file);
        if let Some(line) = related.line {
            out.push_str(&format!(":{}:{}", line, related.column.unwrap_or(1)));
        }
    }
    out
}

/// Renders the trailing summary, or `None` when no error was reported.
pub fn summary(diagnostics: &[ReportedDiagnostic]) -> Option<String> {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == "error")
        .count();
    match errors {
        0 => None,
        1 => Some("error: aborting due to previous error".to_owned()),
        count => Some(format!("error: aborting due to {count} previous errors")),
    }
}

#[cfg(test)]
mod tests {
    use super::{render, snippet, summary};
    use crate::reported_json;

    fn diagnostic() -> crate::ReportedDiagnostic {
        reported_json(
            "NS4001",
            "error",
            "Type mismatch: expected 'i32', found 'str'.",
            Some("Convert the value explicitly."),
            Some("src/main.ns"),
            Some(1),
            Some(9),
        )
        .with_span(Some((8, 13)))
        .with_snippet("let x = \"two\"\n")
    }

    #[test]
    fn snippet_underlines_the_span() {
        let rendered = snippet("let x = \"two\"\n", 8, 13, 1).unwrap();
        assert_eq!(rendered, "  |\n1 | let x = \"two\"\n  |         ^^^^^");
    }

    #[test]
    fn render_includes_location_snippet_and_hint() {
        let expected = [
            "error[NS4001]: Type mismatch: expected 'i32', found 'str'.",
            " --> src/main.ns:1:9",
            "  |",
            "1 | let x = \"two\"",
            "  |         ^^^^^",
            "  |",
            "  = help: Convert the value explicitly.",
        ]
        .join("\n");
        assert_eq!(render(&diagnostic()), expected);
    }

    #[test]
    fn render_includes_related_notes() {
        let mut diagnostic = diagnostic();
        diagnostic.related = vec![crate::ReportedRelated {
            file: "src/main.ns".to_owned(),
            start: 0,
            end: 10,
            message: "Function defined here.".to_owned(),
            line: Some(3),
            column: Some(1),
        }];
        let rendered = render(&diagnostic);
        assert!(
            rendered.ends_with("note: Function defined here.\n --> src/main.ns:3:1"),
            "{rendered}"
        );
    }

    #[test]
    fn summary_counts_only_errors() {
        let mut warning = diagnostic();
        warning.severity = "warning".to_owned();
        assert_eq!(summary(&[warning]), None);
        assert_eq!(
            summary(&[diagnostic()]).as_deref(),
            Some("error: aborting due to previous error")
        );
        assert_eq!(
            summary(&[diagnostic(), diagnostic()]).as_deref(),
            Some("error: aborting due to 2 previous errors")
        );
    }
}
