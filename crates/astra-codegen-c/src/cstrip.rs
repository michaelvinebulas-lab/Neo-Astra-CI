/// Removes C and C++ comments from generated source while preserving string
/// and character literals. Newlines inside block comments are retained so
/// diagnostics and line-oriented tooling keep their original line numbers.
pub(crate) fn strip_comments(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' || c == '\'' {
            let quote = c;
            out.push(c);
            i += 1;
            while i < chars.len() {
                let current = chars[i];
                out.push(current);
                i += 1;
                if current == '\\' {
                    if i < chars.len() {
                        out.push(chars[i]);
                        i += 1;
                    }
                    continue;
                }
                if current == quote {
                    break;
                }
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            out.push(' ');
            let mut closed = false;
            while i + 1 < chars.len() {
                if chars[i] == '*' && chars[i + 1] == '/' {
                    i += 2;
                    closed = true;
                    break;
                }
                if chars[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            if !closed {
                while i < chars.len() {
                    if chars[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_comments;

    #[test]
    fn removes_line_and_block_comments() {
        let input = "int a; // one\nint b; /* two\nthree */ int c;\n";
        let output = strip_comments(input);
        assert!(!output.contains("one"));
        assert!(!output.contains("two"));
        assert!(!output.contains("three"));
        assert!(output.contains("int a;"));
        assert!(output.contains("int b;"));
        assert!(output.contains("int c;"));
        assert_eq!(output.lines().count(), input.lines().count());
    }

    #[test]
    fn preserves_comment_markers_inside_literals() {
        let input = "const char* a = \"http://example.test/*not a comment*/\";\nchar b = '/';\nchar c = '\"';\n";
        let output = strip_comments(input);
        assert!(output.contains("http://example.test/*not a comment*/"));
        assert!(output.contains("char b = '/';"));
        assert!(output.contains("char c = '\"';"));
    }

    #[test]
    fn removes_comments_between_tokens_without_joining_them() {
        let output = strip_comments("int/**/value;");
        assert_eq!(output, "int value;");
    }
}
