use super::{
    verify_update_metadata_with_options, ArtifactUrlPolicy, MetadataRequirements, TrustedUpdateKey,
    UpdateArtifactQuery, VerificationOptions, VerifiedUpdateArtifact,
};
use hbb_common::{anyhow::Context, bail, ResultType};
use serde_derive::Deserialize;

#[derive(Clone, Copy)]
pub(crate) struct OfflineUpdateMetadataRequirements<'a> {
    pub local_file_name: &'a str,
    pub expected_package_id: &'a str,
    pub artifact: UpdateArtifactQuery<'a>,
}

pub(super) struct OfflineVerificationOptions<'a> {
    pub(super) requirements: OfflineUpdateMetadataRequirements<'a>,
    pub(super) trusted_keys: &'a [TrustedUpdateKey],
}

#[derive(Deserialize)]
struct OfflineMetadataHeader {
    version: String,
    release_id: String,
}

pub(super) fn verify_offline_update_metadata_with_options(
    metadata_bytes: &[u8],
    signature_bytes: &[u8],
    options: OfflineVerificationOptions<'_>,
) -> ResultType<VerifiedUpdateArtifact> {
    let header: OfflineMetadataHeader =
        serde_json::from_slice(metadata_bytes).context("invalid update metadata JSON")?;
    let expected_version = crate::common::display_version_from_release_id(&header.release_id)?;
    if header.version != expected_version {
        bail!("offline update metadata version and release id mismatch");
    }
    let artifact = verify_update_metadata_with_options(
        metadata_bytes,
        signature_bytes,
        VerificationOptions {
            requirements: MetadataRequirements {
                expected_version: &expected_version,
                expected_release_id: &header.release_id,
                artifact: options.requirements.artifact,
            },
            expected_package_id: options.requirements.expected_package_id,
            artifact_url_policy: ArtifactUrlPolicy::SignedHttps,
            trusted_keys: options.trusted_keys,
        },
    )?;
    if artifact.file_name != options.requirements.local_file_name {
        bail!("local update file name does not match signed metadata");
    }
    Ok(artifact)
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_offline_update_metadata(
    metadata_bytes: &[u8],
    signature_bytes: &[u8],
    requirements: OfflineUpdateMetadataRequirements<'_>,
) -> ResultType<VerifiedUpdateArtifact> {
    verify_offline_update_metadata_with_options(
        metadata_bytes,
        signature_bytes,
        OfflineVerificationOptions {
            requirements,
            trusted_keys: super::TRUSTED_UPDATE_KEYS,
        },
    )
}
