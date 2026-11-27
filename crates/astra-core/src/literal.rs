//! Decoding of validated string and byte-string tokens.

fn decode_escape(inner: &str, index: &mut usize, is_byte: bool) -> Result<Vec<u8>, String> {
    let Some(character) = inner[*index..].chars().next() else {
        return Err("escape at end of literal".to_owned());
    };
    *index += character.len_utf8();

    match character {
        '\\' => Ok(vec![b'\\']),
        '"' => Ok(vec![b'"']),
        '\'' => Ok(vec![b'\'']),
        'n' => Ok(vec![b'\n']),
        'r' => Ok(vec![b'\r']),
        't' => Ok(vec![b'\t']),
        '0' => Ok(vec![0]),
        'u' => {
            if !inner[*index..].starts_with('{') {
                return Err("expected '{' after \\u".to_owned());
            }
            *index += 1;

            let digits_start = *index;
            while let Some(next) = inner[*index..].chars().next() {
                if next == '}' {
                    break;
                }
                *index += next.len_utf8();
            }

            if !inner[*index..].starts_with('}') {
                return Err("unterminated Unicode escape".to_owned());
            }

            let digits = &inner[digits_start..*index];
            *index += 1;

            if digits.is_empty()
                || digits.len() > 6
                || !digits.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err("invalid Unicode escape digits".to_owned());
            }

            let value =
                u32::from_str_radix(digits, 16).map_err(|_| "invalid Unicode value".to_owned())?;
            let decoded =
                char::from_u32(value).ok_or_else(|| "invalid Unicode scalar value".to_owned())?;

            if is_byte {
                if value > 0xFF {
                    return Err("Unicode escape does not fit in u8".to_owned());
                }
                Ok(vec![value as u8])
            } else {
                let mut buffer = [0_u8; 4];
                Ok(decoded.encode_utf8(&mut buffer).as_bytes().to_vec())
            }
        }
        other => Err(format!("invalid escape \\{other}")),
    }
}

/// Decodes a validated string literal token.
pub fn decode_string_literal(raw: &str) -> Result<String, String> {
    if !raw.starts_with('"') || !raw.ends_with('"') || raw.len() < 2 {
        return Err("invalid string literal token".to_owned());
    }

    let inner = &raw[1..raw.len() - 1];
    let mut output = String::new();
    let mut index = 0;

    while index < inner.len() {
        let character = inner[index..]
            .chars()
            .next()
            .ok_or_else(|| "invalid string literal".to_owned())?;
        index += character.len_utf8();

        if character == '\\' {
            let bytes = decode_escape(inner, &mut index, false)?;
            output.push_str(std::str::from_utf8(&bytes).map_err(|error| error.to_string())?);
        } else {
            output.push(character);
        }
    }

    Ok(output)
}

/// Decodes a validated character literal token such as `'a'` or `'\u{1F600}'`.
pub fn decode_char_literal(raw: &str) -> Result<char, String> {
    let decoded = decode_string_literal(&format!("\"{}\"", &raw[1..raw.len() - 1]))?;
    let mut characters = decoded.chars();
    let value = characters
        .next()
        .ok_or_else(|| "empty character literal".to_owned())?;
    if characters.next().is_some() {
        return Err("character literal contains more than one scalar value".to_owned());
    }
    Ok(value)
}

/// Decodes a validated byte-string literal token such as `b"abc"`.
pub fn decode_byte_literal(raw: &str) -> Result<Vec<u8>, String> {
    if !raw.starts_with("b\"") || !raw.ends_with('"') || raw.len() < 3 {
        return Err("invalid byte literal token".to_owned());
    }

    let inner = &raw[2..raw.len() - 1];
    let mut output = Vec::new();
    let mut index = 0;

    while index < inner.len() {
        let character = inner[index..]
            .chars()
            .next()
            .ok_or_else(|| "invalid byte literal".to_owned())?;
        index += character.len_utf8();

        if character == '\\' {
            output.extend(decode_escape(inner, &mut index, true)?);
        } else {
            let value = character as u32;
            if value > 0xFF {
                return Err("byte literal contains a value outside u8".to_owned());
            }
            output.push(value as u8);
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{decode_byte_literal, decode_string_literal};

    #[test]
    fn decodes_string_escapes() {
        assert_eq!(
            decode_string_literal(r#""hello\nworld""#).unwrap(),
            "hello\nworld"
        );
        assert_eq!(decode_string_literal(r#""\u{1F600}""#).unwrap(), "😀");
    }

    #[test]
    fn decodes_byte_escapes() {
        assert_eq!(
            decode_byte_literal(r#"b"a\n\x""#).unwrap_err(),
            "invalid escape \\x"
        );
        assert_eq!(decode_byte_literal(r#"b"a\n""#).unwrap(), b"a\n");
    }
}
