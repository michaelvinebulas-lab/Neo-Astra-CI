use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::package_id::locale_cmp;

pub struct RegistryPayload<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub dependencies: &'a [(String, String)],
    pub targets: &'a [String],
    pub capabilities: &'a [String],
    pub integrity: &'a str,
    pub resolved: &'a str,
    pub lifecycle_scripts: bool,
}

/// Rebuilds the exact JSON payload the registry signs. Key order and array
/// sorting follow `registryPayload` in the TypeScript oracle.
pub fn registry_payload(payload: &RegistryPayload<'_>) -> String {
    let mut dependencies: Vec<&(String, String)> = payload.dependencies.iter().collect();
    dependencies.sort_by(|left, right| locale_cmp(&left.0, &right.0));
    let mut targets: Vec<&String> = payload.targets.iter().collect();
    targets.sort();
    let mut capabilities: Vec<&String> = payload.capabilities.iter().collect();
    capabilities.sort();

    let mut output = String::from("{\"name\":");
    output.push_str(&serde_json::to_string(payload.name).unwrap_or_default());
    output.push_str(",\"version\":");
    output.push_str(&serde_json::to_string(payload.version).unwrap_or_default());
    output.push_str(",\"dependencies\":{");
    for (index, (name, spec)) in dependencies.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&serde_json::to_string(name).unwrap_or_default());
        output.push(':');
        output.push_str(&serde_json::to_string(spec).unwrap_or_default());
    }
    output.push_str("},\"targets\":[");
    for (index, target) in targets.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&serde_json::to_string(target).unwrap_or_default());
    }
    output.push_str("],\"capabilities\":[");
    for (index, capability) in capabilities.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&serde_json::to_string(capability).unwrap_or_default());
    }
    output.push_str("],\"integrity\":");
    output.push_str(&serde_json::to_string(payload.integrity).unwrap_or_default());
    output.push_str(",\"resolved\":");
    output.push_str(&serde_json::to_string(payload.resolved).unwrap_or_default());
    output.push_str(",\"lifecycleScripts\":");
    output.push_str(if payload.lifecycle_scripts {
        "true"
    } else {
        "false"
    });
    output.push('}');
    output
}

pub fn verify_registry_package(
    payload: &RegistryPayload<'_>,
    signature: &str,
    public_key: &str,
) -> bool {
    let Ok(signature_bytes) = base64::engine::general_purpose::STANDARD.decode(signature) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&signature_bytes) else {
        return false;
    };
    let Some(key_bytes) = parse_ed25519_spki(public_key) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key_bytes) else {
        return false;
    };
    key.verify(payload_text(payload).as_bytes(), &signature)
        .is_ok()
}

fn payload_text(payload: &RegistryPayload<'_>) -> String {
    registry_payload(payload)
}

fn parse_ed25519_spki(public_key: &str) -> Option<[u8; 32]> {
    let body: String = public_key
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    let der = base64::engine::general_purpose::STANDARD
        .decode(body)
        .ok()?;
    const PREFIX: [u8; 12] = [
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
    ];
    if der.len() != 44 || der[..12] != PREFIX {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&der[12..]);
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn public_key_pem(key: &SigningKey) -> String {
        let mut der = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        der.extend_from_slice(key.verifying_key().as_bytes());
        let body = base64::engine::general_purpose::STANDARD.encode(der);
        format!("-----BEGIN PUBLIC KEY-----\n{body}\n-----END PUBLIC KEY-----\n")
    }

    #[test]
    fn payload_order_matches_the_oracle() {
        let dependencies = vec![
            ("zeta".to_owned(), "npm:z@1".to_owned()),
            ("alpha".to_owned(), "npm:a@1".to_owned()),
        ];
        let targets = vec!["web".to_owned(), "native".to_owned()];
        let capabilities = vec!["dom".to_owned(), "fs".to_owned()];
        let payload = RegistryPayload {
            name: "demo",
            version: "1.0.0",
            dependencies: &dependencies,
            targets: &targets,
            capabilities: &capabilities,
            integrity: "sha256:abc",
            resolved: "https://example.test/demo.tgz",
            lifecycle_scripts: false,
        };
        assert_eq!(
            registry_payload(&payload),
            "{\"name\":\"demo\",\"version\":\"1.0.0\",\"dependencies\":{\"alpha\":\"npm:a@1\",\"zeta\":\"npm:z@1\"},\"targets\":[\"native\",\"web\"],\"capabilities\":[\"dom\",\"fs\"],\"integrity\":\"sha256:abc\",\"resolved\":\"https://example.test/demo.tgz\",\"lifecycleScripts\":false}"
        );
    }

    #[test]
    fn verifies_ed25519_signatures() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let payload = RegistryPayload {
            name: "demo",
            version: "1.0.0",
            dependencies: &[],
            targets: &[],
            capabilities: &[],
            integrity: "sha256:abc",
            resolved: "https://example.test/demo.tgz",
            lifecycle_scripts: false,
        };
        let signature = base64::engine::general_purpose::STANDARD.encode(
            signing
                .sign(registry_payload(&payload).as_bytes())
                .to_bytes(),
        );
        assert!(verify_registry_package(
            &payload,
            &signature,
            &public_key_pem(&signing)
        ));
        assert!(!verify_registry_package(&payload, "not-base64", "bad"));
    }
}
