use crate::common::SOFTWARE_UPDATE_URL;
use hbb_common::{log, tls::TlsType, tokio, ResultType};
use serde_json::Value;

pub fn repo() -> Option<&'static str> {
    option_env!("FORK_UPDATE_REPO").filter(|s| !s.is_empty())
}

fn current_tag() -> Option<&'static str> {
    option_env!("FORK_RELEASE_TAG").filter(|s| !s.is_empty())
}

pub fn check() {
    std::thread::spawn(|| hbb_common::allow_err!(do_check()));
}

#[tokio::main(flavor = "current_thread")]
async fn do_check() -> ResultType<()> {
    let (Some(repo), Some(current)) = (repo(), current_tag()) else {
        return Ok(());
    };
    let client = crate::hbbs_http::create_http_client_async(TlsType::Rustls, false);
    let release: Value = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("User-Agent", "rustdesk-fork-update")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let tag = release["tag_name"].as_str().unwrap_or_default();
    let url = release["html_url"].as_str().unwrap_or_default();
    if tag.is_empty() || tag == current || !url.starts_with(&format!("https://github.com/{repo}/")) {
        *SOFTWARE_UPDATE_URL.lock().unwrap() = String::new();
        return Ok(());
    }
    *SOFTWARE_UPDATE_URL.lock().unwrap() = url.to_owned();
    #[cfg(feature = "flutter")]
    {
        let m = std::collections::HashMap::from([
            ("name", "check_software_update_finish"),
            ("url", url),
        ]);
        if let Ok(data) = serde_json::to_string(&m) {
            let _ = crate::flutter::push_global_event(crate::flutter::APP_TYPE_MAIN, data);
        }
    }
    Ok(())
}
