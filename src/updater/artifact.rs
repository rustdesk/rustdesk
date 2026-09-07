use crate::{
    common::{display_version_from_release_id, release_id_from_update_url},
    hbbs_http::create_http_client_with_url_strict,
    update_metadata::{UpdateArtifactQuery, UpdateMetadataRequirements, VerifiedUpdateArtifact},
};
use hbb_common::{bail, ResultType};
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

const UPDATE_SIDECAR_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const UPDATE_METADATA_SIDECAR_MAX_BYTES: u64 = 1024 * 1024;

lazy_static::lazy_static! {
    static ref VERIFIED_UPDATE_ARTIFACT: Mutex<Option<VerifiedUpdateArtifact>> = Mutex::new(None);
}

pub(crate) fn current_update_platform() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::consts::OS
    }
}

pub(crate) fn current_update_arch() -> &'static str {
    #[cfg(all(target_os = "windows", not(feature = "flutter")))]
    {
        "x86"
    }
    #[cfg(not(all(target_os = "windows", not(feature = "flutter"))))]
    {
        std::env::consts::ARCH
    }
}

pub(crate) fn current_update_format(update_msi: bool) -> &'static str {
    #[cfg(any(
        not(target_os = "windows"),
        all(target_os = "windows", not(feature = "flutter"))
    ))]
    let _ = update_msi;
    #[cfg(all(target_os = "windows", feature = "flutter"))]
    {
        if update_msi {
            return "msi";
        }
        "exe"
    }
    #[cfg(all(target_os = "windows", not(feature = "flutter")))]
    {
        "exe"
    }
    #[cfg(target_os = "macos")]
    {
        "dmg"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "unknown"
    }
}

pub fn current_update_artifact_query(update_msi: bool) -> UpdateArtifactQuery<'static> {
    UpdateArtifactQuery {
        platform: current_update_platform(),
        arch: current_update_arch(),
        format: current_update_format(update_msi),
    }
}

pub fn verified_update_artifact_for_release_page_url(
    release_page_url: &str,
    query: UpdateArtifactQuery<'_>,
) -> ResultType<VerifiedUpdateArtifact> {
    let artifact = verified_update_artifact_from_release_page_url(release_page_url, &query)?;
    *VERIFIED_UPDATE_ARTIFACT.lock().unwrap() = Some(artifact.clone());
    Ok(artifact)
}

pub fn verified_update_artifact_for_download_url(
    download_url: &str,
) -> ResultType<VerifiedUpdateArtifact> {
    VERIFIED_UPDATE_ARTIFACT
        .lock()
        .unwrap()
        .as_ref()
        .filter(|artifact| artifact.url == download_url)
        .cloned()
        .ok_or_else(|| hbb_common::anyhow::anyhow!("update artifact was not verified"))
}

pub(super) fn verified_update_artifact_from_release_page_url(
    update_url: &str,
    query: &UpdateArtifactQuery<'_>,
) -> ResultType<VerifiedUpdateArtifact> {
    let release_id = release_id_from_update_url(update_url)?;
    let display_version = display_version_from_release_id(&release_id)?;
    let expected_artifact_url_prefix =
        format!("https://github.com/rustdesk/rustdesk/releases/download/{release_id}/");
    let metadata_url = format!("{expected_artifact_url_prefix}rustdesk-update.json");
    let signature_url = format!("{metadata_url}.sig");
    let metadata_bytes = fetch_update_sidecar_bytes(&metadata_url)?;
    let signature_bytes = fetch_update_sidecar_bytes(&signature_url)?;
    let requirements = UpdateMetadataRequirements {
        expected_version: display_version.as_str(),
        expected_release_id: release_id.as_str(),
        expected_artifact_url_prefix: expected_artifact_url_prefix.as_str(),
        artifact: *query,
    };
    crate::update_metadata::verify_update_metadata(&metadata_bytes, &signature_bytes, requirements)
}

fn fetch_update_sidecar_bytes(url: &str) -> ResultType<Vec<u8>> {
    let client = create_http_client_with_url_strict(url)?;
    let mut response = client
        .get(url)
        .timeout(UPDATE_SIDECAR_HTTP_REQUEST_TIMEOUT)
        .send()?;
    if !response.status().is_success() {
        bail!(
            "Failed to download update metadata sidecar: {}",
            response.status()
        );
    }
    read_limited_response_bytes(
        &mut response,
        UPDATE_METADATA_SIDECAR_MAX_BYTES,
        "Update metadata sidecar",
    )
}

fn read_limited_response_bytes<R: Read>(
    reader: &mut R,
    limit: u64,
    what: &str,
) -> ResultType<Vec<u8>> {
    let mut limited_reader = reader.take(limit.saturating_add(1));
    let mut bytes = Vec::new();
    limited_reader.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("{what} exceeds maximum allowed size of {limit} bytes");
    }
    Ok(bytes)
}

pub fn get_update_download_file_from_url(url: &str) -> Option<PathBuf> {
    let parsed = url::Url::parse(url).ok()?;
    // Check the raw prefix before Url normalizes default ports.
    if !url.starts_with("https://github.com/")
        || parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }

    let mut segments = parsed.path_segments()?;
    let owner = segments.next()?;
    let repo = segments.next()?;
    let releases = segments.next()?;
    let download = segments.next()?;
    let tag = segments.next()?;
    let filename = segments.next()?;

    if owner != "rustdesk"
        || repo != "rustdesk"
        || releases != "releases"
        || download != "download"
        || tag.is_empty()
        || segments.next().is_some()
        || !is_plain_update_filename(filename)
    {
        return None;
    }

    Some(std::env::temp_dir().join(filename))
}

fn is_plain_update_filename(filename: &str) -> bool {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains(':')
    {
        return false;
    }

    let mut components = Path::new(filename).components();
    matches!(
        components.next(),
        Some(Component::Normal(name)) if name.to_str() == Some(filename)
    ) && components.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_download_file_accepts_expected_github_asset_urls() {
        let file = get_update_download_file_from_url(
            "https://github.com/rustdesk/rustdesk/releases/download/1.4.0/rustdesk-1.4.0-x86_64.dmg",
        )
        .expect("valid GitHub release asset URL");

        assert_eq!(
            file.file_name().and_then(|name| name.to_str()),
            Some("rustdesk-1.4.0-x86_64.dmg")
        );
    }

    #[test]
    fn update_download_file_rejects_untrusted_or_malformed_urls() {
        for url in [
            "http://github.com/rustdesk/rustdesk/releases/download/1/rustdesk.exe",
            "https://example.com/rustdesk.exe",
            "https://github.com/other/project/releases/download/1/rustdesk.exe",
            "https://github.com/rustdesk/rustdesk/releases/download/1/",
            "https://github.com/rustdesk/rustdesk/releases/download/1/nested/rustdesk.exe",
            "https://github.com/rustdesk/rustdesk/releases/download/1/C:rustdesk.exe",
            "https://user@github.com/rustdesk/rustdesk/releases/download/1/rustdesk.exe",
            "https://github.com:443/rustdesk/rustdesk/releases/download/1/rustdesk.exe",
            "https://github.com/rustdesk/rustdesk/releases/download/1/rustdesk.exe?download=1",
            "https://github.com/rustdesk/rustdesk/releases/download/1/rustdesk.exe#download",
            "not a url",
        ] {
            assert!(get_update_download_file_from_url(url).is_none(), "{url}");
        }
    }

    #[test]
    fn limited_sidecar_reader_rejects_oversized_payloads() {
        let mut payload: &[u8] = b"rustdesk";
        assert_eq!(
            read_limited_response_bytes(&mut payload, 8, "sidecar")
                .unwrap()
                .len(),
            8
        );

        let mut oversized: &[u8] = b"too-large";
        assert!(read_limited_response_bytes(&mut oversized, 4, "sidecar").is_err());
    }
}
