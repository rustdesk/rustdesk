use crate::common::SOFTWARE_UPDATE_URL;
use hbb_common::{
    base64::{engine::general_purpose::STANDARD, Engine as _},
    get_version_number,
    tls::TlsType,
    ResultType,
};
use serde_json::Value;

const UPSTREAM_REPO: &str = "rustdesk/rustdesk";

// Both are baked in by the release workflow. Without them the stock update flow is untouched.
fn fork_repo() -> Option<&'static str> {
    option_env!("FORK_UPDATE_REPO").filter(|s| !s.is_empty())
}

pub fn trusted_public_key() -> Option<[u8; 32]> {
    let encoded = option_env!("FORK_UPDATE_PUBLIC_KEY").filter(|s| !s.is_empty())?;
    STANDARD.decode(encoded).ok()?.try_into().ok()
}

pub fn is_fork_build() -> bool {
    fork_repo().is_some() && trusted_public_key().is_some()
}

pub fn update_repo() -> &'static str {
    match fork_repo() {
        Some(repo) if trusted_public_key().is_some() => repo,
        _ => UPSTREAM_REPO,
    }
}

pub async fn check() -> ResultType<()> {
    let client = crate::hbbs_http::create_http_client_async(TlsType::Rustls, false);
    let release: Value = client
        .get(format!(
            "https://api.github.com/repos/{}/releases/latest",
            update_repo()
        ))
        .header("User-Agent", "rustdesk-fork-update")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let tag = release["tag_name"].as_str().unwrap_or_default();
    let url = format!("https://github.com/{}/releases/tag/{tag}", update_repo());
    let is_newer = crate::common::release_id_from_update_url(&url)
        .and_then(|id| crate::common::display_version_from_release_id(&id))
        .map(|v| get_version_number(&v) > get_version_number(crate::VERSION))
        .unwrap_or(false);
    if !is_newer {
        *SOFTWARE_UPDATE_URL.lock().unwrap() = String::new();
        return Ok(());
    }
    #[cfg(feature = "flutter")]
    {
        let m = std::collections::HashMap::from([
            ("name", "check_software_update_finish"),
            ("url", url.as_str()),
        ]);
        if let Ok(data) = serde_json::to_string(&m) {
            let _ = crate::flutter::push_global_event(crate::flutter::APP_TYPE_MAIN, data);
        }
    }
    *SOFTWARE_UPDATE_URL.lock().unwrap() = url;
    Ok(())
}
