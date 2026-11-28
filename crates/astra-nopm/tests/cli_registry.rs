use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

fn serve(responses: Vec<(u16, &'static str)>) -> (u16, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&request);
            let path = text
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("")
                .to_owned();
            requests.push(path);
            let reason = if status == 200 {
                "OK"
            } else {
                "Internal Server Error"
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (port, handle)
}

fn nopm(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_nopm"))
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn search_preserves_registry_order_and_text_output() {
    let (port, server) = serve(vec![(
        200,
        r#"{"results":[{"name":"demo","version":"1.0.0"}],"total":1}"#,
    )]);
    let registry = format!("http://127.0.0.1:{port}");
    let (code, stdout, stderr) = nopm(&["search", "demo query", "--registry", &registry, "--json"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        stdout,
        "{\n  \"ok\": true,\n  \"command\": \"search\",\n  \"result\": {\n    \"results\": [\n      {\n        \"name\": \"demo\",\n        \"version\": \"1.0.0\"\n      }\n    ],\n    \"total\": 1\n  }\n}\n"
    );
    assert_eq!(server.join().unwrap(), vec!["/v1/search?q=demo%20query"]);

    let (port, server) = serve(vec![(
        200,
        r#"{"results":[{"name":"demo","version":"1.0.0"}]}"#,
    )]);
    let registry = format!("http://127.0.0.1:{port}");
    let (code, stdout, stderr) = nopm(&["search", "demo", "--registry", &registry]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "demo@1.0.0\n");
    server.join().unwrap();
}

#[test]
fn audit_preserves_registry_order_and_text_output() {
    let (port, server) = serve(vec![(
        200,
        r#"{"yanked":false,"advisories":[{"id":"A-1"}]}"#,
    )]);
    let registry = format!("http://127.0.0.1:{port}");
    let (code, stdout, stderr) = nopm(&[
        "audit",
        "nova:demo@1.0.0",
        "--registry",
        &registry,
        "--json",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        stdout,
        "{\n  \"ok\": true,\n  \"command\": \"audit\",\n  \"result\": {\n    \"yanked\": false,\n    \"advisories\": [\n      {\n        \"id\": \"A-1\"\n      }\n    ]\n  }\n}\n"
    );
    assert_eq!(server.join().unwrap(), vec!["/v1/audit/demo/1.0.0"]);

    let (port, server) = serve(vec![(200, r#"{"yanked":true,"advisories":[]}"#)]);
    let registry = format!("http://127.0.0.1:{port}");
    let (code, stdout, stderr) = nopm(&["audit", "nova:demo@1.0.0", "--registry", &registry]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "demo@1.0.0: yanked, 0 advisory(ies)\n");
    server.join().unwrap();
}

#[test]
fn registry_failures_use_the_oracle_codes() {
    let (port, server) = serve(vec![(500, "{}")]);
    let registry = format!("http://127.0.0.1:{port}");
    let (code, stdout, _) = nopm(&["search", "demo", "--registry", &registry, "--json"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        "{\n  \"ok\": false,\n  \"diagnostics\": [\n    {\n      \"code\": \"NP0011\",\n      \"message\": \"Registry search failed: HTTP 500.\",\n      \"path\": \"http://127.0.0.1:PORT\"\n    }\n  ]\n}\n"
            .replace("PORT", &port.to_string())
    );
    server.join().unwrap();

    let (code, _, stderr) = nopm(&["search", "demo"]);
    assert_eq!(code, 2);
    assert_eq!(
        stderr,
        "NP0012: No registry is configured. Pass --registry <url> or set [registry].default.\n"
    );
}

#[test]
fn audit_requires_an_exact_version() {
    let (code, _, stderr) = nopm(&["audit", "npm:demo", "--registry", "http://127.0.0.1:1"]);
    assert_eq!(code, 2);
    assert_eq!(
        stderr,
        "NP0012: nopm audit requires an exact package version.\n"
    );
}
