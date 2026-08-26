use super::UpdateArtifact;
use hbb_common::{
    anyhow::{anyhow, Context},
    ResultType,
};
use url::Url;

#[derive(Clone, Copy)]
pub(super) enum ArtifactUrlPolicy<'a> {
    ExactPrefix(&'a str),
    #[cfg(any(target_os = "macos", test))]
    SignedHttps,
}

pub(super) fn validate_artifact(
    artifact: &UpdateArtifact,
    url_policy: ArtifactUrlPolicy<'_>,
) -> ResultType<()> {
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
    validate_artifact_url_policy(artifact, &parsed_url, url_policy)?;
    let basename = parsed_url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .ok_or_else(|| anyhow!("update artifact URL has no basename"))?;
    if basename != artifact.file_name {
        return Err(anyhow!("update artifact URL basename mismatch"));
    }
    Ok(())
}

fn validate_artifact_url_policy(
    artifact: &UpdateArtifact,
    _parsed_url: &Url,
    policy: ArtifactUrlPolicy<'_>,
) -> ResultType<()> {
    match policy {
        ArtifactUrlPolicy::ExactPrefix(prefix) => {
            let expected_url = format!("{}{}", prefix, artifact.file_name);
            if artifact.url != expected_url {
                return Err(anyhow!(
                    "update artifact URL is outside expected release prefix"
                ));
            }
        }
        #[cfg(any(target_os = "macos", test))]
        ArtifactUrlPolicy::SignedHttps => {
            if _parsed_url.scheme() != "https"
                || _parsed_url.host_str().is_none()
                || !_parsed_url.username().is_empty()
                || _parsed_url.password().is_some()
            {
                return Err(anyhow!(
                    "signed update artifact URL must be an HTTPS URL without credentials"
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_artifact_file_name(file_name: &str) -> ResultType<()> {
    if file_name.is_empty()
        || matches!(file_name, "." | "..")
        || !file_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(anyhow!("invalid update artifact file name"));
    }
    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}
