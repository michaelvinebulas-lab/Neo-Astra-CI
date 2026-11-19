use std::io::Read;

use base64::Engine;
use serde_json::Value;

use crate::error::NopmError;

pub const USER_AGENT: &str = "neo-astra-nopm/0.1.0";

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Blocking HTTP seam. The production implementation is [`UreqClient`]; tests
/// inject deterministic responses without opening sockets.
pub trait HttpClient {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UreqClient;

impl HttpClient for UreqClient {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, String> {
        if let Some(rest) = url.strip_prefix("data:") {
            return data_url(rest);
        }

        let mut request = ureq::get(url).set("User-Agent", USER_AGENT);
        for (name, value) in headers {
            request = request.set(name, value);
        }
        match request.call() {
            Ok(response) => read_response(response),
            Err(ureq::Error::Status(_, response)) => read_response(response),
            Err(ureq::Error::Transport(error)) => Err(error.to_string()),
        }
    }
}

fn read_response(response: ureq::Response) -> Result<HttpResponse, String> {
    let status = response.status();
    let mut body = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    Ok(HttpResponse { status, body })
}

fn data_url(rest: &str) -> Result<HttpResponse, String> {
    let Some((metadata, payload)) = rest.split_once(',') else {
        return Err("Invalid data URL.".to_owned());
    };
    let body = if metadata.ends_with(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|error| error.to_string())?
    } else {
        percent_decode(payload)
    };
    Ok(HttpResponse { status: 200, body })
}

/// Matches JavaScript's `encodeURIComponent`: alphanumerics and `-_.!~*'()`
/// stay literal; every other UTF-8 byte is percent-encoded with uppercase hex.
pub fn encode_uri_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        let character = *byte as char;
        if character.is_ascii_alphanumeric()
            || matches!(
                character,
                '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')'
            )
        {
            encoded.push(character);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

fn percent_decode(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            decoded.push(high * 16 + low);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    decoded
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Fetches a JSON object with the oracle's exact failure messages.
pub fn get_json(client: &dyn HttpClient, url: &str, action: &str) -> Result<Value, NopmError> {
    let response = client
        .get(url, &[("Accept", "application/json")])
        .map_err(|message| {
            NopmError::with_path("NP0011", format!("Unable to {action}: {message}"), url)
        })?;
    if !response.ok() {
        return Err(NopmError::with_path(
            "NP0011",
            format!("Unable to {action}: HTTP {}.", response.status),
            url,
        ));
    }
    let value: Value = serde_json::from_slice(&response.body).map_err(|_| {
        NopmError::with_path(
            "NP0011",
            format!("Unable to parse JSON while trying to {action}."),
            url,
        )
    })?;
    if !value.is_object() {
        return Err(NopmError::with_path(
            "NP0011",
            format!("Unexpected JSON while trying to {action}."),
            url,
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    struct MockClient {
        responses: BTreeMap<String, HttpResponse>,
    }

    impl HttpClient for MockClient {
        fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<HttpResponse, String> {
            self.responses
                .get(url)
                .cloned()
                .ok_or_else(|| "unexpected url".to_owned())
        }
    }

    fn response(value: Value) -> HttpResponse {
        HttpResponse {
            status: 200,
            body: serde_json::to_vec(&value).unwrap(),
        }
    }

    #[test]
    fn encode_uri_component_matches_javascript() {
        assert_eq!(encode_uri_component("react"), "react");
        assert_eq!(encode_uri_component("@scope/pkg"), "%40scope%2Fpkg");
        assert_eq!(encode_uri_component("a b+c"), "a%20b%2Bc");
        assert_eq!(encode_uri_component("!'()*~"), "!'()*~");
    }

    #[test]
    fn data_urls_decode_base64_payloads() {
        let client = UreqClient;
        let response = client
            .get("data:application/gzip;base64,aGVsbG8=", &[])
            .unwrap();
        assert_eq!(response.body, b"hello");
    }

    #[test]
    fn get_json_reports_http_and_shape_failures() {
        let mut responses = BTreeMap::new();
        responses.insert(
            "https://example.test/ok".to_owned(),
            response(json!({"ok": true})),
        );
        responses.insert(
            "https://example.test/status".to_owned(),
            HttpResponse {
                status: 404,
                body: Vec::new(),
            },
        );
        responses.insert("https://example.test/array".to_owned(), response(json!([])));
        let client = MockClient { responses };

        assert!(get_json(&client, "https://example.test/ok", "read metadata").is_ok());
        let status = get_json(&client, "https://example.test/status", "read metadata").unwrap_err();
        assert_eq!(status.code, "NP0011");
        assert_eq!(status.message, "Unable to read metadata: HTTP 404.");
        let shape = get_json(&client, "https://example.test/array", "read metadata").unwrap_err();
        assert_eq!(
            shape.message,
            "Unexpected JSON while trying to read metadata."
        );
    }
}
