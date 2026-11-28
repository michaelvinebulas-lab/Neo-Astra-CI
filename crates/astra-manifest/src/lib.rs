use astra_core::diagnostic::Diagnostic;
use astra_core::edition::Edition;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

const CORE_KEYS: [&str; 6] = [
    "package",
    "dependencies",
    "packageSources",
    "registry",
    "ffi",
    "capabilities",
];

const BUILD_KEYS: [&str; 7] = [
    "build",
    "targets",
    "nova",
    "math",
    "strings",
    "memory",
    "toolchain",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackageManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CoreManifest {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<PackageManifest>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_sources: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ffi: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<serde_json::Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildManifest {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildSection>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub targets: BTreeMap<String, TargetSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nova: Option<NovaSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub math: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strings: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<serde_json::Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_dir: Option<String>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triple: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasi: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NovaSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<bool>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn invalid(code: &str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(code, message)]
}

fn parse_core_value(value: serde_json::Value) -> Result<CoreManifest, Vec<Diagnostic>> {
    if !value.is_object() {
        return Err(invalid("NS9002", "Manifest root must be an object."));
    }
    let manifest: CoreManifest = serde_json::from_value(value)
        .map_err(|error| invalid("NS9001", format!("invalid core manifest: {error}")))?;
    reject_cross_manifest_keys(&manifest.extra, false)?;
    if let Some(edition) = manifest
        .package
        .as_ref()
        .and_then(|package| package.edition.as_deref())
        && let Err(message) = edition.parse::<Edition>()
    {
        return Err(vec![
            Diagnostic::error("NS2008", message).with_hint("Set edition to \"0.1\" or \"0.2\"."),
        ]);
    }
    Ok(manifest)
}

fn parse_build_value(value: serde_json::Value) -> Result<BuildManifest, Vec<Diagnostic>> {
    if !value.is_object() {
        return Err(invalid("NS9002", "Manifest root must be an object."));
    }
    let manifest: BuildManifest = serde_json::from_value(value)
        .map_err(|error| invalid("NS9001", format!("invalid build manifest: {error}")))?;
    reject_cross_manifest_keys(&manifest.extra, true)?;
    Ok(manifest)
}

fn reject_cross_manifest_keys(
    extra: &BTreeMap<String, serde_json::Value>,
    build: bool,
) -> Result<(), Vec<Diagnostic>> {
    for key in extra.keys() {
        let wrong = if build {
            CORE_KEYS.contains(&key.as_str())
        } else {
            BUILD_KEYS.contains(&key.as_str())
        };
        if wrong {
            return Err(invalid(
                "NS9002",
                format!(
                    "Key '{key}' belongs in {}.json, not {}.",
                    if build { "core" } else { "build" },
                    if build { "build" } else { "core" }
                ),
            ));
        }
    }
    Ok(())
}

pub fn parse_core_json(text: &str) -> Result<CoreManifest, Vec<Diagnostic>> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| invalid("NS9001", format!("invalid core manifest: {error}")))?;
    parse_core_value(value)
}

pub fn parse_build_json(text: &str) -> Result<BuildManifest, Vec<Diagnostic>> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| invalid("NS9001", format!("invalid build manifest: {error}")))?;
    parse_build_value(value)
}

pub fn parse_core_toml(text: &str) -> Result<CoreManifest, Vec<Diagnostic>> {
    let value: toml::Value = toml::from_str(text)
        .map_err(|error| invalid("NS9001", format!("invalid core manifest: {error}")))?;
    let value = serde_json::to_value(value)
        .map_err(|error| invalid("NS9001", format!("invalid core manifest: {error}")))?;
    parse_core_value(value)
}

pub fn parse_build_toml(text: &str) -> Result<BuildManifest, Vec<Diagnostic>> {
    let value: toml::Value = toml::from_str(text)
        .map_err(|error| invalid("NS9001", format!("invalid build manifest: {error}")))?;
    let value = serde_json::to_value(value)
        .map_err(|error| invalid("NS9001", format!("invalid build manifest: {error}")))?;
    parse_build_value(value)
}

pub fn format_core_json(m: &CoreManifest) -> String {
    serde_json::to_string_pretty(m).unwrap_or_default()
}

pub fn format_build_json(m: &BuildManifest) -> String {
    serde_json::to_string_pretty(m).unwrap_or_default()
}

pub fn format_core_toml(text: &str) -> Result<String, Vec<Diagnostic>> {
    format_toml(text)
}

pub fn format_build_toml(text: &str) -> Result<String, Vec<Diagnostic>> {
    format_toml(text)
}

fn format_toml(text: &str) -> Result<String, Vec<Diagnostic>> {
    if !text.contains('#') {
        let value: toml::Value = toml::from_str(text)
            .map_err(|error| invalid("NS9002", format!("Invalid TOML manifest: {error}")))?;
        let formatted = toml::to_string_pretty(&value)
            .map_err(|error| invalid("NS9002", format!("Invalid TOML manifest: {error}")))?;
        return Ok(format!("{}\n", formatted.trim_end()));
    }
    let document = toml_edit::DocumentMut::from_str(text)
        .map_err(|error| invalid("NS9002", format!("Invalid TOML manifest: {error}")))?;
    Ok(format!("{}\n", document.to_string().trim_end()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_core_json_accepts_oracle_root_and_package_fields() {
        let text = r#"{
            "$schema": "https://example.invalid/core.schema.json",
            "package": {
                "name": "foo",
                "authors": ["A"],
                "repository": "https://example.invalid/repo",
                "keywords": ["cli"]
            },
            "dependencies": {
                "local": { "source": "path", "path": "local" }
            },
            "ffi": { "native": { "native": { "source": "native", "type": "c" } } }
        }"#;
        let manifest = parse_core_json(text).unwrap();
        assert_eq!(
            manifest.package.as_ref().unwrap().name.as_deref(),
            Some("foo")
        );
        assert!(manifest.schema.is_some());
        assert!(manifest.dependencies.contains_key("local"));
    }

    #[test]
    fn parse_core_json_allows_an_absent_package() {
        let manifest = parse_core_json("{}").unwrap();
        assert!(manifest.package.is_none());
    }

    #[test]
    fn parse_core_json_accepts_supported_editions() {
        for edition in ["0.1", "0.2"] {
            let text = format!(r#"{{"package":{{"name":"foo","edition":"{edition}"}}}}"#);
            let manifest = parse_core_json(&text).unwrap();
            assert_eq!(
                manifest.package.as_ref().unwrap().edition.as_deref(),
                Some(edition)
            );
        }
    }

    #[test]
    fn parse_core_json_rejects_unsupported_edition_with_a_migration_hint() {
        let error = parse_core_json(r#"{"package":{"edition":"2026"}}"#).unwrap_err();
        assert!(error.iter().any(|diagnostic| {
            diagnostic.code == "NS2008"
                && diagnostic
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.contains("\"0.1\" or \"0.2\""))
        }));
    }

    #[test]
    fn parse_core_json_rejects_build_keys() {
        let error = parse_core_json(r#"{"build":{"defaultTarget":"native"}}"#).unwrap_err();
        assert!(error.iter().any(|diagnostic| diagnostic.code == "NS9002"));
    }

    #[test]
    fn parse_core_json_invalid() {
        let error = parse_core_json("{bad").unwrap_err();
        assert!(error.iter().any(|diagnostic| diagnostic.code == "NS9001"));
    }

    #[test]
    fn parse_build_json_accepts_oracle_target_and_nova_fields() {
        let text = r#"{
            "build": { "defaultTarget": "web", "defaultMode": "web" },
            "targets": {
                "web": {
                    "backend": "js",
                    "module": "esm",
                    "triple": "wasm32",
                    "mode": "web",
                    "outDir": "dist/web",
                    "wasi": false,
                    "env": { "FOO": "bar" }
                }
            },
            "nova": { "entry": "src/main.ns", "watch": true },
            "math": { "integers": "wrapping" }
        }"#;
        let manifest = parse_build_json(text).unwrap();
        assert_eq!(
            manifest.build.as_ref().unwrap().default_target.as_deref(),
            Some("web")
        );
        assert!(manifest.targets["web"].module.is_some());
        assert_eq!(manifest.nova.as_ref().unwrap().watch, Some(true));
    }

    #[test]
    fn format_core_json_keeps_schema_first_and_unknown_fields() {
        let manifest =
            parse_core_json(r#"{"package":{"name":"foo"},"x-extra":true,"$schema":"schema"}"#)
                .unwrap();
        let formatted = format_core_json(&manifest);
        assert!(formatted.starts_with("{\n  \"$schema\": \"schema\""));
        assert!(formatted.contains("\"x-extra\": true"));
    }

    #[test]
    fn format_toml_preserves_comments_and_adds_newline() {
        let text = "# header\n[package]\nname=\"foo\"\n";
        let formatted = format_core_toml(text).unwrap();
        assert!(formatted.contains("# header"));
        assert!(formatted.contains("name=\"foo\""));
        assert!(formatted.ends_with('\n'));
    }
}
