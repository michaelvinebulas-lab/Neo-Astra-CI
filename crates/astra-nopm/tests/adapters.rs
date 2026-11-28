use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use astra_nopm::adapters;
use astra_nopm::command::{CommandOutput, CommandRunner, command_name};
use astra_nopm::http::{HttpClient, HttpResponse};
use astra_nopm::package_id::{format_package_id, parse_package_id};

struct MockClient {
    responses: BTreeMap<String, HttpResponse>,
}

impl HttpClient for MockClient {
    fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<HttpResponse, String> {
        self.responses
            .get(url)
            .cloned()
            .ok_or_else(|| format!("unexpected url: {url}"))
    }
}

fn json_response(value: serde_json::Value) -> HttpResponse {
    HttpResponse {
        status: 200,
        body: serde_json::to_vec(&value).unwrap(),
    }
}

fn client(responses: &[(&str, HttpResponse)]) -> MockClient {
    MockClient {
        responses: responses
            .iter()
            .map(|(url, response)| ((*url).to_owned(), response.clone()))
            .collect(),
    }
}

struct QueueRunner {
    outputs: Mutex<VecDeque<CommandOutput>>,
    calls: Mutex<Vec<String>>,
}

impl QueueRunner {
    fn new(outputs: Vec<CommandOutput>) -> Self {
        Self {
            outputs: Mutex::new(outputs.into()),
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl CommandRunner for QueueRunner {
    fn run(&self, command: &str, args: &[&str], _inherit: bool) -> CommandOutput {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{command} {}", args.join(" ")).trim().to_owned());
        self.outputs.lock().unwrap().pop_front().unwrap_or_default()
    }
}

fn success(stdout: &str) -> CommandOutput {
    CommandOutput {
        status: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn cargo_resolves_ranges_dependencies_and_metadata() {
    let client = client(&[
        (
            "https://crates.io/api/v1/crates/demo",
            json_response(serde_json::json!({
                "crate": {"max_stable_version": "1.2.0"},
                "versions": [
                    {"num": "1.2.0"},
                    {"num": "1.3.0-beta.1"},
                    {"num": "0.9.0", "yanked": true}
                ]
            })),
        ),
        (
            "https://crates.io/api/v1/crates/demo/1.2.0",
            json_response(serde_json::json!({
                "version": {"num": "1.2.0", "checksum": "abc", "license": "MIT"}
            })),
        ),
        (
            "https://crates.io/api/v1/crates/demo/1.2.0/dependencies",
            json_response(serde_json::json!({
                "dependencies": [
                    {"kind": "normal", "crate_id": "dep", "req": "^2"},
                    {"kind": "dev", "crate_id": "dev-dep", "req": "^1"},
                    {"kind": "normal", "optional": true, "crate_id": "opt", "req": "^3"}
                ]
            })),
        ),
    ]);
    let id = parse_package_id("cargo:demo@^1.0").unwrap();
    let resolution = adapters::cargo::resolve_cargo_package(&id, "demo", &client, None).unwrap();
    assert_eq!(
        format_package_id(&resolution.descriptor.id),
        "cargo:demo@1.2.0"
    );
    assert_eq!(
        resolution.descriptor.integrity.as_deref(),
        Some("sha256:abc")
    );
    assert_eq!(
        resolution.descriptor.resolved.as_deref(),
        Some("https://static.crates.io/crates/demo/demo-1.2.0.crate")
    );
    assert_eq!(
        resolution.descriptor.registry.as_deref(),
        Some("https://crates.io")
    );
    assert_eq!(
        resolution.descriptor.dependencies,
        vec![("dep".to_owned(), "^2".to_owned())]
    );
    assert_eq!(
        format_package_id(&resolution.request.id),
        "cargo:demo@1.2.0"
    );
}

#[test]
fn pypi_prefers_sdists_and_only_keeps_exact_requirements() {
    let client = client(&[(
        "https://pypi.org/pypi/Demo_Pkg/1.0.0/json",
        json_response(serde_json::json!({
            "info": {
                "name": "demo-pkg",
                "version": "1.0.0",
                "requires_dist": ["dep==2.0.0", "range>=1", "marker; python_version < '3.12'"]
            },
            "urls": [
                {
                    "packagetype": "bdist_wheel",
                    "url": "https://files.example/demo.whl",
                    "digests": {"sha256": "wheel"}
                },
                {
                    "packagetype": "sdist",
                    "url": "https://files.example/demo.tar.gz",
                    "digests": {"sha256": "sdist"}
                }
            ]
        })),
    )]);
    let id = parse_package_id("pypi:Demo_Pkg@1.0.0").unwrap();
    let resolution = adapters::pypi::resolve_pypi_package(&id, "demo", &client, None).unwrap();
    assert_eq!(
        format_package_id(&resolution.descriptor.id),
        "pypi:demo-pkg@1.0.0"
    );
    assert_eq!(
        resolution.descriptor.resolved.as_deref(),
        Some("https://files.example/demo.tar.gz")
    );
    assert_eq!(
        resolution.descriptor.integrity.as_deref(),
        Some("sha256:sdist")
    );
    assert_eq!(
        resolution.descriptor.dependencies,
        vec![
            ("dep".to_owned(), "pypi:dep@2.0.0".to_owned()),
            ("marker".to_owned(), "pypi:marker".to_owned())
        ]
    );
}

#[test]
fn nuget_reads_flat_container_integrity_and_registration_dependencies() {
    let client = client(&[
        (
            "https://api.nuget.org/v3-flatcontainer/demo/1.0.0/demo.1.0.0.nupkg.sha512",
            HttpResponse {
                status: 200,
                body: b"ABC\n".to_vec(),
            },
        ),
        (
            "https://api.nuget.org/v3/registration5-gz-semver2/demo/1.0.0.json",
            json_response(serde_json::json!({
                "catalogEntry": {
                    "dependencyGroups": [
                        {"dependencies": [
                            {"id": "Dep", "range": "[2.0.0]"},
                            {"id": "Range", "range": "[1.0.0,2.0.0)"}
                        ]}
                    ]
                }
            })),
        ),
    ]);
    let id = parse_package_id("nuget:Demo@1.0.0").unwrap();
    let resolution = adapters::nuget::resolve_nuget_package(&id, "demo", &client, None).unwrap();
    assert_eq!(
        format_package_id(&resolution.descriptor.id),
        "nuget:Demo@1.0.0"
    );
    assert_eq!(
        resolution.descriptor.integrity.as_deref(),
        Some("sha512-ABC")
    );
    assert_eq!(
        resolution.descriptor.resolved.as_deref(),
        Some("https://api.nuget.org/v3-flatcontainer/demo/1.0.0/demo.1.0.0.nupkg")
    );
    assert_eq!(
        resolution.descriptor.dependencies,
        vec![("Dep".to_owned(), "nuget:Dep@2.0.0".to_owned())]
    );
}

#[test]
fn oci_selects_the_tar_layer_and_uses_its_digest() {
    let client = client(&[(
        "http://localhost:5000/v2/demo/manifests/v1",
        json_response(serde_json::json!({
            "layers": [
                {"mediaType": "application/vnd.oci.image.config.v1+json", "digest": "sha256:config"},
                {"mediaType": "application/vnd.oci.image.layer.v1.tar", "digest": "SHA256:ABCDEF"}
            ]
        })),
    )]);
    let id = parse_package_id("oci:localhost:5000/demo@v1").unwrap();
    let resolution = adapters::oci::resolve_oci_package(&id, "demo", &client).unwrap();
    assert_eq!(
        format_package_id(&resolution.descriptor.id),
        "oci:localhost:5000/demo@v1"
    );
    assert_eq!(
        resolution.descriptor.integrity.as_deref(),
        Some("sha256:abcdef")
    );
    assert_eq!(
        resolution.descriptor.resolved.as_deref(),
        Some("http://localhost:5000/v2/demo/blobs/SHA256:ABCDEF")
    );
    assert_eq!(
        resolution.descriptor.registry.as_deref(),
        Some("http://localhost:5000")
    );
}

#[test]
fn npm_uses_npm_view_and_records_dist_metadata() {
    let runner = QueueRunner::new(vec![
        success("\"19.1.0\"\n"),
        success(
            &serde_json::json!({
                "name": "react",
                "version": "19.1.0",
                "dependencies": {"scheduler": "^0.23.0"},
                "dist": {
                    "tarball": "https://registry.npmjs.org/react/-/react-19.1.0.tgz",
                    "integrity": "sha512-react"
                }
            })
            .to_string(),
        ),
    ]);
    let id = parse_package_id("npm:react@19").unwrap();
    let resolution = adapters::npm::resolve_npm_package(&id, "react", &runner).unwrap();
    assert_eq!(
        format_package_id(&resolution.descriptor.id),
        "npm:react@19.1.0"
    );
    assert_eq!(
        resolution.descriptor.integrity.as_deref(),
        Some("sha512-react")
    );
    assert_eq!(
        resolution.descriptor.registry.as_deref(),
        Some("https://registry.npmjs.org")
    );
    assert_eq!(
        resolution.descriptor.dependencies,
        vec![("scheduler".to_owned(), "^0.23.0".to_owned())]
    );
    let calls = runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0],
        format!("{} view react@19 version --json", command_name("npm"))
    );
    assert_eq!(
        calls[1],
        format!("{} view react@19.1.0 --json", command_name("npm"))
    );
}

#[test]
fn git_reads_the_pinned_cache_directory() {
    let directory = tempfile::tempdir().unwrap();
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let cache = directory.path().join(".astra").join("cache");
    let package = cache.join("git").join(commit);
    write(
        &package.join("core.json"),
        "{\"package\":{\"name\":\"gitpkg\",\"version\":\"1.0.0\"},\"dependencies\":{\"dep\":\"path:../dep\"},\"capabilities\":{\"web\":[\"fs\"]}}",
    );
    write(
        &package.join("build.json"),
        "{\"targets\":{\"web\":{\"backend\":\"js\",\"mode\":\"web\"}}}",
    );
    let runner = QueueRunner::new(Vec::new());
    let id = parse_package_id(&format!("git+https://example.com/repo#{commit}")).unwrap();
    let resolution = adapters::git::resolve_git_package(&id, "gitpkg", &cache, &runner).unwrap();
    assert_eq!(
        format_package_id(&resolution.descriptor.id),
        format!("git+https://example.com/repo#{commit}")
    );
    assert_eq!(resolution.descriptor.commit.as_deref(), Some(commit));
    let expected_integrity = format!("git:{commit}");
    assert_eq!(
        resolution.descriptor.integrity.as_deref(),
        Some(expected_integrity.as_str())
    );
    assert_eq!(
        resolution.descriptor.resolved.as_deref(),
        Some("https://example.com/repo")
    );
    assert_eq!(resolution.descriptor.targets, vec!["web".to_owned()]);
    assert_eq!(resolution.descriptor.capabilities, vec!["fs".to_owned()]);
    assert_eq!(
        resolution.descriptor.dependencies,
        vec![("dep".to_owned(), "path:../dep".to_owned())]
    );
}

#[test]
fn nova_dependencies_keep_first_position_and_last_value_for_duplicates() {
    let client = client(&[(
        "https://registry.example/v1/packages/demo/1.0.0",
        HttpResponse {
            status: 200,
            body: br#"{"version":"1.0.0","integrity":"sha256:abc","dependencies":{"b":"npm:b@1","a":"npm:a@1","b":"npm:b@2"}}"#.to_vec(),
        },
    )]);
    let id = parse_package_id("nova:demo@1.0.0").unwrap();
    let resolution = adapters::registry::resolve_registry_package(
        &id,
        "demo",
        &client,
        adapters::registry::RegistryOptions {
            registry: "https://registry.example",
            token: None,
            require_signature: false,
            public_key: None,
        },
    )
    .unwrap();
    assert_eq!(
        resolution.descriptor.dependencies,
        vec![
            ("b".to_owned(), "npm:b@2".to_owned()),
            ("a".to_owned(), "npm:a@1".to_owned())
        ]
    );
}

#[test]
fn nova_reads_descriptors_and_rejects_version_mismatches() {
    let good_client = client(&[(
        "https://registry.example/v1/packages/demo/1.0.0",
        json_response(serde_json::json!({
            "name": "demo",
            "version": "1.0.0",
            "integrity": "sha256:abc",
            "resolved": "https://registry.example/demo.tgz",
            "dependencies": {"dep": "npm:dep@1.0.0"},
            "targets": ["web"],
            "capabilities": ["fs"],
            "lifecycleScripts": false
        })),
    )]);
    let id = parse_package_id("nova:demo@1.0.0").unwrap();
    let resolution = adapters::registry::resolve_registry_package(
        &id,
        "demo",
        &good_client,
        adapters::registry::RegistryOptions {
            registry: "https://registry.example/",
            token: None,
            require_signature: false,
            public_key: None,
        },
    )
    .unwrap();
    assert_eq!(
        format_package_id(&resolution.descriptor.id),
        "nova:demo@1.0.0"
    );
    assert_eq!(
        resolution.descriptor.registry.as_deref(),
        Some("https://registry.example/")
    );
    assert_eq!(resolution.descriptor.targets, vec!["web".to_owned()]);
    assert_eq!(resolution.descriptor.capabilities, vec!["fs".to_owned()]);
    assert_eq!(
        resolution.descriptor.dependencies,
        vec![("dep".to_owned(), "npm:dep@1.0.0".to_owned())]
    );

    let bad_client = client(&[(
        "https://registry.example/v1/packages/demo/1.0.0",
        json_response(serde_json::json!({
            "version": "2.0.0",
            "integrity": "sha256:abc"
        })),
    )]);
    let error = adapters::registry::resolve_registry_package(
        &id,
        "demo",
        &bad_client,
        adapters::registry::RegistryOptions {
            registry: "https://registry.example",
            token: None,
            require_signature: false,
            public_key: None,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "Registry returned version '2.0.0' for 'nova:demo@1.0.0'."
    );
}
