use super::*;
use hbb_common::base64::engine::general_purpose::STANDARD;
use hbb_common::{
    sodiumoxide::{self, crypto::sign},
    ResultType,
};
use serde_json::{json, Value};

const KEY_ID: &str = "test-ed25519-main";
const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Fixture {
    metadata: Vec<u8>,
    signature: Vec<u8>,
    public_key: [u8; 32],
}

fn artifact() -> Value {
    json!({"platform":"windows","arch":"x86_64","format":"exe","url":"https://github.com/rustdesk/rustdesk/releases/download/v1.4.6/rustdesk-1.4.6-x86_64.exe","file_name":"rustdesk-1.4.6-x86_64.exe","size":123456,"sha256":SHA256})
}

fn metadata() -> Value {
    json!({"schema_version":1,"app":"rustdesk","package_id":"rustdesk","version":"1.4.6","release_id":"v1.4.6","published_at":"2026-05-14T00:00:00Z","signature_key_id":KEY_ID,"artifacts":[artifact()]})
}

fn set_value(metadata: &mut Value, key: &str, value: Value, artifact_field: bool) {
    let target = if artifact_field {
        &mut metadata["artifacts"][0]
    } else {
        metadata
    };
    target
        .as_object_mut()
        .expect("JSON object")
        .insert(key.to_string(), value);
}

fn sign_fixture(metadata: Value, signature_key_id: &str) -> Fixture {
    sodiumoxide::init().expect("test sodiumoxide init");
    let metadata = serde_json::to_vec(&metadata).expect("metadata JSON");
    let (public_key, secret_key) = sign::gen_keypair();
    let mut signed = UPDATE_METADATA_SIGNATURE_CONTEXT.to_vec();
    signed.extend_from_slice(&metadata);
    let sig = sign::sign_detached(&signed, &secret_key);
    let sig_json = json!({"schema_version":1,"algorithm":"ed25519","key_id":signature_key_id,"signature":STANDARD.encode(sig.to_bytes())});
    Fixture {
        metadata,
        signature: serde_json::to_vec(&sig_json).expect("signature JSON"),
        public_key: public_key.0,
    }
}

fn valid_fixture() -> Fixture {
    sign_fixture(metadata(), KEY_ID)
}
fn key(public_key: [u8; 32]) -> TrustedUpdateKey {
    TrustedUpdateKey {
        key_id: KEY_ID,
        algorithm: "ed25519",
        public_key,
    }
}
fn query<'a>() -> UpdateArtifactQuery<'a> {
    UpdateArtifactQuery {
        platform: "windows",
        arch: "x86_64",
        format: "exe",
    }
}
fn requirements<'a>(artifact: UpdateArtifactQuery<'a>) -> MetadataRequirements<'a> {
    MetadataRequirements {
        expected_version: "1.4.6",
        expected_release_id: "v1.4.6",
        artifact,
    }
}
fn verify_with_query(
    fixture: &Fixture,
    query: &UpdateArtifactQuery<'_>,
) -> ResultType<VerifiedUpdateArtifact> {
    let trusted_keys = [key(fixture.public_key)];
    verify_update_metadata_with_options(
        &fixture.metadata,
        &fixture.signature,
        VerificationOptions {
            requirements: requirements(*query),
            expected_package_id: PUBLIC_UPDATE_PACKAGE_ID,
            artifact_url_policy: ArtifactUrlPolicy::ExactPrefix(
                "https://github.com/rustdesk/rustdesk/releases/download/v1.4.6/",
            ),
            trusted_keys: &trusted_keys,
        },
    )
}

fn verify(fixture: &Fixture) -> ResultType<VerifiedUpdateArtifact> {
    verify_with_query(fixture, &query())
}

fn verify_for_package(
    fixture: &Fixture,
    expected_package_id: &str,
) -> ResultType<VerifiedUpdateArtifact> {
    let trusted_keys = [key(fixture.public_key)];
    verify_update_metadata_with_options(
        &fixture.metadata,
        &fixture.signature,
        VerificationOptions {
            requirements: requirements(query()),
            expected_package_id,
            artifact_url_policy: ArtifactUrlPolicy::ExactPrefix(
                "https://github.com/rustdesk/rustdesk/releases/download/v1.4.6/",
            ),
            trusted_keys: &trusted_keys,
        },
    )
}

#[test]
fn accepts_valid_metadata_and_signature() {
    let artifact = verify(&valid_fixture()).expect("verified artifact");
    assert_eq!("rustdesk-1.4.6-x86_64.exe", artifact.file_name);
    assert_eq!(123456, artifact.size);
    assert_eq!(SHA256, artifact.sha256);
    assert_eq!("1.4.6", artifact.version);
}

#[test]
fn accepts_only_the_expected_package_identity() {
    const CUSTOM_PACKAGE_ID: &str = "com.example.rustdesk-custom";
    let mut custom_metadata = metadata();
    custom_metadata["package_id"] = json!(CUSTOM_PACKAGE_ID);
    let fixture = sign_fixture(custom_metadata, KEY_ID);

    assert!(verify_for_package(&fixture, CUSTOM_PACKAGE_ID).is_ok());
    assert!(verify_for_package(&fixture, "rustdesk").is_err());
}

#[test]
fn rejects_bad_signature_or_tampered_metadata() {
    let mut fixture = valid_fixture();
    fixture.metadata.push(b' ');
    assert!(verify(&fixture).is_err());
    for (key, value) in [
        ("schema_version", json!(2)),
        ("algorithm", json!("rsa")),
        ("signature", json!("not base64")),
        ("signature", json!(STANDARD.encode([1u8; 63]))),
    ] {
        let mut fixture = valid_fixture();
        let mut sig: Value = serde_json::from_slice(&fixture.signature).expect("signature");
        sig.as_object_mut()
            .expect("signature object")
            .insert(key.to_string(), value);
        fixture.signature = serde_json::to_vec(&sig).expect("signature bytes");
        assert!(verify(&fixture).is_err(), "{}", key);
    }
}

#[test]
fn rejects_schema_policy_artifact_and_url_mismatches() {
    let cases = [
        ("schema_version", json!(2), false), ("app", json!("other"), false),
        ("version", json!("1.4.7"), false), ("release_id", json!("v1.4.7"), false),
        ("package_id", json!("custom"), false), ("signature_key_id", json!("different-key"), false),
        ("arch", json!("x86"), true),
        ("sha256", json!("not-a-sha256"), true),
        ("url", json!("https://github.com/rustdesk/rustdesk/releases/download/v1.4.7/rustdesk-1.4.6-x86_64.exe"), true),
        ("url", json!("https://github.com/rustdesk/rustdesk/releases/download/v1.4.6/other.exe"), true),
    ];
    for (key, value, artifact_field) in cases {
        let mut metadata = metadata();
        set_value(&mut metadata, key, value, artifact_field);
        assert!(verify(&sign_fixture(metadata, KEY_ID)).is_err(), "{}", key);
    }
}

#[test]
fn rejects_artifact_url_query_or_fragment() {
    for suffix in ["?download=1", "#hash"] {
        let mut invalid_metadata = metadata();
        invalid_metadata["artifacts"][0]["url"] = json!(format!(
            "https://github.com/rustdesk/rustdesk/releases/download/v1.4.6/rustdesk-1.4.6-x86_64.exe{suffix}"
        ));
        let fixture = sign_fixture(invalid_metadata, KEY_ID);
        let error = verify(&fixture).err().expect("invalid artifact URL");

        assert_eq!(
            "update artifact URL must not contain query or fragment",
            error.to_string()
        );
    }
}

#[test]
fn rejects_duplicate_artifact_matches() {
    let mut duplicate_metadata = metadata();
    duplicate_metadata["artifacts"]
        .as_array_mut()
        .expect("artifacts")
        .push(artifact());
    assert!(verify(&sign_fixture(duplicate_metadata, KEY_ID)).is_err());
}

#[test]
fn rejects_artifact_file_name_path_components() {
    for file_name in [
        "",
        "nested/rustdesk.exe",
        r"nested\rustdesk.exe",
        "..",
        "rust desk.exe",
        "rustdesk*.exe",
        "rustdesk\n.exe",
        "rüstdesk.exe",
    ] {
        let mut invalid_metadata = metadata();
        invalid_metadata["artifacts"][0]["file_name"] = json!(file_name);
        invalid_metadata["artifacts"][0]["url"] = json!(format!(
            "https://github.com/rustdesk/rustdesk/releases/download/v1.4.6/{file_name}"
        ));
        let fixture = sign_fixture(invalid_metadata, KEY_ID);

        let error = verify_with_query(&fixture, &query())
            .err()
            .expect("invalid file name");

        assert_eq!("invalid update artifact file name", error.to_string());
    }
}

#[test]
fn rejects_path_components_in_unselected_artifact_file_name() {
    let mut invalid_metadata = metadata();
    let mut unselected_artifact = artifact();
    unselected_artifact["platform"] = json!("linux");
    unselected_artifact["file_name"] = json!("nested/rustdesk-linux.exe");
    invalid_metadata["artifacts"]
        .as_array_mut()
        .expect("artifacts")
        .push(unselected_artifact);

    let error = verify(&sign_fixture(invalid_metadata, KEY_ID))
        .err()
        .expect("invalid file name");

    assert_eq!("invalid update artifact file name", error.to_string());
}

#[test]
fn verifies_metadata_generated_by_python_release_script_fixture() {
    const PYTHON_KEY_ID: &str = "python-test-ed25519-main";
    let metadata = br#"{"schema_version":1,"app":"rustdesk","package_id":"rustdesk","version":"1.4.6","release_id":"v1.4.6","published_at":"2026-05-14T00:00:00Z","signature_key_id":"python-test-ed25519-main","artifacts":[{"platform":"windows","arch":"x86_64","format":"exe","url":"https://github.com/rustdesk/rustdesk/releases/download/v1.4.6/rustdesk-1.4.6-x86_64.exe","file_name":"rustdesk-1.4.6-x86_64.exe","size":8,"sha256":"304ca1638c5effa6832e0e15b958a8f74847efe4df9c3f3187216e921c168fed"}]}"#;
    let signature = br#"{"schema_version":1,"algorithm":"ed25519","key_id":"python-test-ed25519-main","signature":"ZMcod9VNaEGTYK0gIfGmMQ44HAvrYAkYDyIL9JTKjzUH+hVFYs8KpvnGSlteAHwqiJuJDdiKBVCPEdtcRRAJCA=="}"#;
    let trusted_key = TrustedUpdateKey {
        key_id: PYTHON_KEY_ID,
        algorithm: "ed25519",
        public_key: [
            3, 161, 7, 191, 243, 206, 16, 190, 29, 112, 221, 24, 231, 75, 192, 153, 103, 228, 214,
            48, 155, 165, 13, 95, 29, 220, 134, 100, 18, 85, 49, 184,
        ],
    };

    let trusted_keys = [trusted_key];
    let artifact = verify_update_metadata_with_options(
        metadata,
        signature,
        VerificationOptions {
            requirements: requirements(query()),
            expected_package_id: PUBLIC_UPDATE_PACKAGE_ID,
            artifact_url_policy: ArtifactUrlPolicy::ExactPrefix(
                "https://github.com/rustdesk/rustdesk/releases/download/v1.4.6/",
            ),
            trusted_keys: &trusted_keys,
        },
    )
    .expect("Python-generated update metadata verifies");

    assert_eq!("rustdesk-1.4.6-x86_64.exe", artifact.file_name);
}

#[path = "update_metadata/offline_tests.rs"]
mod offline_tests;
