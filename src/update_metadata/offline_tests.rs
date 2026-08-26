use super::*;

fn verify_offline(fixture: &Fixture, local_file_name: &str) -> ResultType<VerifiedUpdateArtifact> {
    verify_offline_for_package(fixture, local_file_name, PUBLIC_UPDATE_PACKAGE_ID)
}

fn verify_offline_for_package(
    fixture: &Fixture,
    local_file_name: &str,
    expected_package_id: &str,
) -> ResultType<VerifiedUpdateArtifact> {
    let trusted_keys = [key(fixture.public_key)];
    verify_offline_update_metadata_with_options(
        &fixture.metadata,
        &fixture.signature,
        OfflineVerificationOptions {
            requirements: OfflineUpdateMetadataRequirements {
                local_file_name,
                expected_package_id,
                artifact: query(),
            },
            trusted_keys: &trusted_keys,
        },
    )
}

#[test]
fn accepts_valid_signed_offline_artifact() {
    let artifact = verify_offline(&valid_fixture(), "rustdesk-1.4.6-x86_64.exe")
        .expect("verified offline artifact");

    assert_eq!("rustdesk-1.4.6-x86_64.exe", artifact.file_name);
    assert_eq!(123456, artifact.size);
    assert_eq!(SHA256, artifact.sha256);
    assert_eq!("1.4.6", artifact.version);
}

#[test]
fn offline_metadata_requires_the_expected_package_identity() {
    const CUSTOM_PACKAGE_ID: &str = "com.example.rustdesk-custom";
    let mut custom_metadata = metadata();
    custom_metadata["package_id"] = json!(CUSTOM_PACKAGE_ID);
    let fixture = sign_fixture(custom_metadata, KEY_ID);

    assert!(
        verify_offline_for_package(&fixture, "rustdesk-1.4.6-x86_64.exe", CUSTOM_PACKAGE_ID)
            .is_ok()
    );
    assert!(verify_offline(&fixture, "rustdesk-1.4.6-x86_64.exe").is_err());
}

#[test]
fn accepts_signed_offline_artifact_from_custom_https_release() {
    let mut custom_metadata = metadata();
    custom_metadata["artifacts"][0]["url"] =
        json!("https://updates.example.com/releases/v1.4.6/rustdesk-1.4.6-x86_64.exe");
    let fixture = sign_fixture(custom_metadata, KEY_ID);

    let artifact = verify_offline(&fixture, "rustdesk-1.4.6-x86_64.exe")
        .expect("verified custom HTTPS artifact");

    assert_eq!("rustdesk-1.4.6-x86_64.exe", artifact.file_name);
}

#[test]
fn rejects_signed_offline_artifact_from_unsafe_url() {
    for url in [
        "http://updates.example.com/rustdesk-1.4.6-x86_64.exe",
        "https://user@updates.example.com/rustdesk-1.4.6-x86_64.exe",
    ] {
        let mut invalid_metadata = metadata();
        invalid_metadata["artifacts"][0]["url"] = json!(url);
        let fixture = sign_fixture(invalid_metadata, KEY_ID);

        assert!(
            verify_offline(&fixture, "rustdesk-1.4.6-x86_64.exe").is_err(),
            "{url}"
        );
    }
}

#[test]
fn rejects_offline_artifact_name_or_release_mismatch() {
    assert!(verify_offline(&valid_fixture(), "renamed-update.exe").is_err());

    let mut invalid_metadata = metadata();
    invalid_metadata["version"] = json!("1.4.7");
    assert!(verify_offline(
        &sign_fixture(invalid_metadata, KEY_ID),
        "rustdesk-1.4.6-x86_64.exe"
    )
    .is_err());
}
