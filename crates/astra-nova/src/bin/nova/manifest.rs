//! Minimal manifest lookup for `nova run` and `nova watch`.
//!
//! Full manifest parsing and formatting remain owned by the TypeScript manifest
//! package. Nova only needs the `[nova].entry` path for its default command
//! target.

use std::env;
use std::fs;
use std::path::Path;

use crate::CliError;

const JSON_MANIFEST: &str = "core.json";
const TOML_MANIFEST: &str = "core.toml";

pub(crate) fn resolve_entry(command: &'static str, json: bool) -> Result<String, CliError> {
    let cwd = env::current_dir().map_err(|error| {
        CliError::input(
            "NS9008",
            format!("Unable to determine the current directory: {error}"),
            command,
            ".".to_owned(),
            json,
        )
    })?;
    let root = fs::canonicalize(&cwd).map_err(|error| {
        CliError::input(
            "NS9008",
            format!("Unable to resolve the project directory: {error}"),
            command,
            cwd.display().to_string(),
            json,
        )
    })?;

    let json_path = root.join(JSON_MANIFEST);
    let toml_path = root.join(TOML_MANIFEST);
    let has_json = json_path.is_file();
    let has_toml = toml_path.is_file();

    let manifest_path = match (has_json, has_toml) {
        (true, false) => json_path,
        (false, true) => toml_path,
        (false, false) => {
            return Err(CliError::input(
                "NS9001",
                "No manifest found. Expected exactly one of core.json or core.toml.",
                command,
                root.display().to_string(),
                json,
            ));
        }
        (true, true) => {
            return Err(CliError::input(
                "NS9001",
                "Both core.json and core.toml exist. Exactly one manifest is allowed.",
                command,
                root.display().to_string(),
                json,
            ));
        }
    };

    let source = fs::read(&manifest_path).map_err(|error| {
        CliError::input(
            "NS9002",
            format!("Unable to read the manifest: {error}"),
            command,
            manifest_path.display().to_string(),
            json,
        )
    })?;
    let source = String::from_utf8(source).map_err(|_| {
        CliError::input(
            "NS9002",
            "The manifest is not valid UTF-8.",
            command,
            manifest_path.display().to_string(),
            json,
        )
    })?;

    let entry = if manifest_path
        .extension()
        .is_some_and(|value| value == "json")
    {
        parse_json_entry(&source, &manifest_path, command, json)?
    } else {
        parse_toml_entry(&source, &manifest_path, command, json)?
    };

    let entry_path = Path::new(&entry);
    if entry_path.is_absolute() {
        return Err(manifest_error(
            "The [nova].entry path must be relative to the project root.",
            &manifest_path,
            command,
            json,
        ));
    }
    if !entry_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("ns"))
    {
        return Err(manifest_error(
            "[nova].entry must reference a .ns file.",
            &manifest_path,
            command,
            json,
        ));
    }

    let resolved = fs::canonicalize(root.join(entry_path)).map_err(|error| {
        CliError::input(
            "NS9008",
            format!("Unable to resolve the [nova].entry file: {error}"),
            command,
            root.join(entry_path).display().to_string(),
            json,
        )
    })?;

    if !resolved.starts_with(&root) {
        return Err(manifest_error(
            "[nova].entry must stay inside the project root.",
            &manifest_path,
            command,
            json,
        ));
    }
    if !resolved.is_file() {
        return Err(CliError::input(
            "NS9008",
            "The [nova].entry target is not a file.",
            command,
            resolved.display().to_string(),
            json,
        ));
    }

    Ok(resolved.display().to_string())
}

fn parse_json_entry(
    source: &str,
    manifest_path: &Path,
    command: &'static str,
    json: bool,
) -> Result<String, CliError> {
    let value: serde_json::Value = serde_json::from_str(source).map_err(|error| {
        CliError::input(
            "NS9002",
            format!("Invalid JSON manifest: {error}"),
            command,
            manifest_path.display().to_string(),
            json,
        )
    })?;

    value
        .get("nova")
        .and_then(|nova| nova.get("entry"))
        .and_then(|entry| entry.as_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            manifest_error(
                "Missing [nova].entry in core.json.",
                manifest_path,
                command,
                json,
            )
        })
}

fn parse_toml_entry(
    source: &str,
    manifest_path: &Path,
    command: &'static str,
    json: bool,
) -> Result<String, CliError> {
    let value: toml::Value = toml::from_str(source).map_err(|error| {
        CliError::input(
            "NS9002",
            format!("Invalid TOML manifest: {error}"),
            command,
            manifest_path.display().to_string(),
            json,
        )
    })?;

    value
        .get("nova")
        .and_then(|nova| nova.get("entry"))
        .and_then(|entry| entry.as_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            manifest_error(
                "Missing [nova].entry in core.toml.",
                manifest_path,
                command,
                json,
            )
        })
}

fn manifest_error(
    message: impl Into<String>,
    manifest_path: &Path,
    command: &'static str,
    json: bool,
) -> CliError {
    CliError::input(
        "NS9002",
        message,
        command,
        manifest_path.display().to_string(),
        json,
    )
}
