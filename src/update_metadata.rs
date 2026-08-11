use hbb_common::{
    anyhow::{anyhow, Context},
    base64::{engine::general_purpose::STANDARD, Engine as _},
    sodiumoxide::{
        self,
        crypto::sign::{verify_detached, PublicKey, Signature},
    },
    ResultType,
};
use serde_derive::Deserialize;
use std::sync::OnceLock;
use url::Url;

const UPDATE_METADATA_SIGNATURE_CONTEXT: &[u8] = b"RustDesk update metadata v1\n";
const UPDATE_APP: &str = "rustdesk";
const UPDATE_PACKAGE_ID: &str = "rustdesk";
const METADATA_SCHEMA_VERSION: u32 = 1;
const SIGNATURE_SCHEMA_VERSION: u32 = 1;
const SIGNATURE_ALGORITHM: &str = "ed25519";

const TRUSTED_UPDATE_KEYS: &[TrustedUpdateKey] = &[TrustedUpdateKey {
    key_id: "2026-ed25519-main",
    algorithm: SIGNATURE_ALGORITHM,
    public_key: [
        118, 252, 116, 215, 49, 217, 220, 109, 145, 119, 210, 101, 225, 198, 63, 2, 41, 127, 197,
        127, 52, 251, 6, 204, 125, 117, 96, 204, 187, 166, 85, 59,
    ],
}];

#[derive(Deserialize)]
struct UpdateMetadata {
    schema_version: u32,
    app: String,
    package_id: String,
    version: String,
    release_id: String,
    #[serde(rename = "published_at")]
    _published_at: String,
    signature_key_id: String,
    artifacts: Vec<UpdateArtifact>,
}

#[derive(Deserialize)]
struct UpdateArtifact {
    platform: String,
    arch: String,
    format: String,
    url: String,
    file_name: String,
    size: u64,
    sha256: String,
}

#[derive(Deserialize)]
struct UpdateSignature {
    schema_version: u32,
    algorithm: String,
    key_id: String,
    signature: String,
}

struct TrustedUpdateKey {
    key_id: &'static str,
    algorithm: &'static str,
    public_key: [u8; 32],
}

#[derive(Clone, Copy)]
pub(crate) struct UpdateArtifactQuery<'a> {
    pub platform: &'a str,
    pub arch: &'a str,
    pub format: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) struct UpdateMetadataRequirements<'a> {
    pub expected_version: &'a str,
    pub expected_release_id: &'a str,
    pub expected_artifact_url_prefix: &'a str,
    pub artifact: UpdateArtifactQuery<'a>,
}

#[derive(Clone)]
pub(crate) struct VerifiedUpdateArtifact {
    pub version: String,
    pub url: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
}

struct VerificationOptions<'a> {
    requirements: UpdateMetadataRequirements<'a>,
    trusted_keys: &'a [TrustedUpdateKey],
}

fn verify_update_metadata_with_options(
    metadata_bytes: &[u8],
    signature_bytes: &[u8],
    options: VerificationOptions<'_>,
) -> ResultType<VerifiedUpdateArtifact> {
    let update_signature: UpdateSignature =
        serde_json::from_slice(signature_bytes).context("invalid update signature JSON")?;
    validate_signature_schema(&update_signature)?;
    let signature = decode_signature(&update_signature.signature)?;
    let trusted_key = options
        .trusted_keys
        .iter()
        .find(|key| {
            key.key_id == update_signature.key_id && key.algorithm == update_signature.algorithm
        })
        .ok_or_else(|| anyhow!("unsupported update signature key"))?;
    verify_metadata_signature(metadata_bytes, &signature, trusted_key)?;
    let metadata: UpdateMetadata =
        serde_json::from_slice(metadata_bytes).context("invalid update metadata JSON")?;
    validate_metadata(&metadata, &update_signature, &options.requirements)?;
    let artifact = select_artifact(&metadata, &options.requirements.artifact)?;
    validate_artifact(artifact, options.requirements.expected_artifact_url_prefix)?;
    Ok(VerifiedUpdateArtifact {
        version: metadata.version.clone(),
        url: artifact.url.clone(),
        file_name: artifact.file_name.clone(),
        size: artifact.size,
        sha256: artifact.sha256.clone(),
    })
}

pub(crate) fn verify_update_metadata(
    metadata_bytes: &[u8],
    signature_bytes: &[u8],
    requirements: UpdateMetadataRequirements<'_>,
) -> ResultType<VerifiedUpdateArtifact> {
    verify_update_metadata_with_options(
        metadata_bytes,
        signature_bytes,
        VerificationOptions {
            requirements,
            trusted_keys: TRUSTED_UPDATE_KEYS,
        },
    )
}

fn validate_signature_schema(signature: &UpdateSignature) -> ResultType<()> {
    if signature.schema_version != SIGNATURE_SCHEMA_VERSION {
        return Err(anyhow!("unsupported update signature schema version"));
    }
    if signature.algorithm != SIGNATURE_ALGORITHM {
        return Err(anyhow!("unsupported update signature algorithm"));
    }
    Ok(())
}

fn decode_signature(encoded: &str) -> ResultType<Signature> {
    let decoded = STANDARD
        .decode(encoded)
        .context("invalid update signature base64")?;
    if decoded.len() != 64 || STANDARD.encode(&decoded) != encoded {
        return Err(anyhow!("invalid update signature length or encoding"));
    }
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&decoded);
    Signature::from_bytes(&signature).map_err(|_| anyhow!("invalid update signature bytes"))
}

fn verify_metadata_signature(
    metadata_bytes: &[u8],
    signature: &Signature,
    trusted_key: &TrustedUpdateKey,
) -> ResultType<()> {
    init_sodiumoxide()?;
    let mut signed_bytes =
        Vec::with_capacity(UPDATE_METADATA_SIGNATURE_CONTEXT.len() + metadata_bytes.len());
    signed_bytes.extend_from_slice(UPDATE_METADATA_SIGNATURE_CONTEXT);
    signed_bytes.extend_from_slice(metadata_bytes);
    if !verify_detached(signature, &signed_bytes, &PublicKey(trusted_key.public_key)) {
        return Err(anyhow!("invalid update metadata signature"));
    }
    Ok(())
}

fn init_sodiumoxide() -> ResultType<()> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    let result = INIT.get_or_init(|| {
        sodiumoxide::init().map_err(|_| "failed to initialize sodiumoxide".to_string())
    });
    match result {
        Ok(()) => Ok(()),
        Err(err) => Err(anyhow!(err.clone())),
    }
}

fn validate_metadata(
    metadata: &UpdateMetadata,
    signature: &UpdateSignature,
    requirements: &UpdateMetadataRequirements<'_>,
) -> ResultType<()> {
    if metadata.schema_version != METADATA_SCHEMA_VERSION {
        return Err(anyhow!("unsupported update metadata schema version"));
    }
    if metadata.signature_key_id != signature.key_id {
        return Err(anyhow!("update metadata signature key id mismatch"));
    }
    if metadata.app != UPDATE_APP {
        return Err(anyhow!("update metadata app mismatch"));
    }
    if metadata.package_id != UPDATE_PACKAGE_ID {
        return Err(anyhow!("update metadata package id is not allowed"));
    }
    if requirements.expected_version != metadata.version {
        return Err(anyhow!("update metadata version mismatch"));
    }
    if requirements.expected_release_id != metadata.release_id {
        return Err(anyhow!("update metadata release id mismatch"));
    }
    for artifact in &metadata.artifacts {
        validate_artifact_file_name(&artifact.file_name)?;
    }
    Ok(())
}

fn select_artifact<'a>(
    metadata: &'a UpdateMetadata,
    query: &UpdateArtifactQuery<'_>,
) -> ResultType<&'a UpdateArtifact> {
    let mut matches = metadata.artifacts.iter().filter(|artifact| {
        artifact.platform == query.platform
            && artifact.arch == query.arch
            && artifact.format == query.format
    });
    let artifact = matches
        .next()
        .ok_or_else(|| anyhow!("matching update artifact not found"))?;
    if matches.next().is_some() {
        return Err(anyhow!("multiple matching update artifacts found"));
    }
    Ok(artifact)
}

fn validate_artifact(artifact: &UpdateArtifact, expected_url_prefix: &str) -> ResultType<()> {
    validate_artifact_file_name(&artifact.file_name)?;
    if !is_sha256_hex(&artifact.sha256) {
        return Err(anyhow!("invalid update artifact sha256"));
    }
    let parsed_url = Url::parse(&artifact.url).context("invalid update artifact URL")?;
    if parsed_url.query().is_some() || parsed_url.fragment().is_some() {
        return Err(anyhow!(
            "update artifact URL must not contain query or fragment"
        ));
    }
    let expected_url = format!("{}{}", expected_url_prefix, artifact.file_name);
    if artifact.url != expected_url {
        return Err(anyhow!(
            "update artifact URL is outside expected release prefix"
        ));
    }
    let basename = parsed_url
        .path_segments()
        .and_then(|segments| segments.last())
        .ok_or_else(|| anyhow!("update artifact URL has no basename"))?;
    if basename != artifact.file_name {
        return Err(anyhow!("update artifact URL basename mismatch"));
    }
    Ok(())
}

fn validate_artifact_file_name(file_name: &str) -> ResultType<()> {
    if file_name.is_empty()
        || matches!(file_name, "." | "..")
        || file_name.contains(['/', '\\', ':'])
    {
        return Err(anyhow!("invalid update artifact file name"));
    }
    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
#[path = "update_metadata_tests.rs"]
mod tests;
