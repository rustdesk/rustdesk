use crate::{common::do_check_software_update, hbbs_http::create_http_client_with_url};
use hbb_common::{bail, config, log, ResultType};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
    time::{Duration, Instant},
};

enum UpdateMsg {
    CheckUpdate,
    Exit,
}

lazy_static::lazy_static! {
    static ref TX_MSG : Mutex<Sender<UpdateMsg>> = Mutex::new(start_auto_update_check());
    static ref DOWNLOAD_FILE_SHA256_CACHE: Mutex<HashMap<String, String>> = Default::default();
}

static CONTROLLING_SESSION_COUNT: AtomicUsize = AtomicUsize::new(0);

const DUR_ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);
const UPDATE_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub fn update_controlling_session_count(count: usize) {
    CONTROLLING_SESSION_COUNT.store(count, Ordering::SeqCst);
}

#[allow(dead_code)]
pub fn start_auto_update() {
    let _sender = TX_MSG.lock().unwrap();
}

#[allow(dead_code)]
pub fn manually_check_update() -> ResultType<()> {
    let sender = TX_MSG.lock().unwrap();
    sender.send(UpdateMsg::CheckUpdate)?;
    Ok(())
}

#[allow(dead_code)]
pub fn stop_auto_update() {
    let sender = TX_MSG.lock().unwrap();
    sender.send(UpdateMsg::Exit).unwrap_or_default();
}

#[inline]
fn has_no_active_conns() -> bool {
    let conns = crate::Connection::alive_conns();
    conns.is_empty() && has_no_controlling_conns()
}

#[cfg(any(not(target_os = "windows"), feature = "flutter"))]
fn has_no_controlling_conns() -> bool {
    CONTROLLING_SESSION_COUNT.load(Ordering::SeqCst) == 0
}

#[cfg(not(any(not(target_os = "windows"), feature = "flutter")))]
fn has_no_controlling_conns() -> bool {
    let app_exe = format!("{}.exe", crate::get_app_name().to_lowercase());
    for arg in [
        "--connect",
        "--play",
        "--file-transfer",
        "--view-camera",
        "--port-forward",
        "--rdp",
    ] {
        if !crate::platform::get_pids_of_process_with_first_arg(&app_exe, arg).is_empty() {
            return false;
        }
    }
    true
}

fn start_auto_update_check() -> Sender<UpdateMsg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || start_auto_update_check_(rx));
    return tx;
}

fn start_auto_update_check_(rx_msg: Receiver<UpdateMsg>) {
    std::thread::sleep(Duration::from_secs(30));
    if let Err(e) = check_update(false) {
        log::error!("Error checking for updates: {}", e);
    }

    const MIN_INTERVAL: Duration = Duration::from_secs(60 * 10);
    const RETRY_INTERVAL: Duration = Duration::from_secs(60 * 30);
    let mut last_check_time = Instant::now();
    let mut check_interval = DUR_ONE_DAY;
    loop {
        let recv_res = rx_msg.recv_timeout(check_interval);
        match &recv_res {
            Ok(UpdateMsg::CheckUpdate) | Err(_) => {
                if last_check_time.elapsed() < MIN_INTERVAL {
                    // log::debug!("Update check skipped due to minimum interval.");
                    continue;
                }
                // Don't check update if there are alive connections.
                if !has_no_active_conns() {
                    check_interval = RETRY_INTERVAL;
                    continue;
                }
                if let Err(e) = check_update(matches!(recv_res, Ok(UpdateMsg::CheckUpdate))) {
                    log::error!("Error checking for updates: {}", e);
                    check_interval = RETRY_INTERVAL;
                } else {
                    last_check_time = Instant::now();
                    check_interval = DUR_ONE_DAY;
                }
            }
            Ok(UpdateMsg::Exit) => break,
        }
    }
}

fn check_update(manually: bool) -> ResultType<()> {
    #[cfg(target_os = "windows")]
    let update_msi = crate::platform::is_msi_installed()? && !crate::is_custom_client();
    if !(manually || config::Config::get_bool_option(config::keys::OPTION_ALLOW_AUTO_UPDATE)) {
        return Ok(());
    }
    if do_check_software_update().is_err() {
        // ignore
        return Ok(());
    }

    let update_url = crate::common::SOFTWARE_UPDATE_URL.lock().unwrap().clone();
    if update_url.is_empty() {
        log::debug!("No update available.");
    } else {
        let download_url = update_url.replace("tag", "download");
        let version = download_url.split('/').last().unwrap_or_default();
        #[cfg(target_os = "windows")]
        let download_url = if cfg!(feature = "flutter") {
            format!(
                "{}/rustdesk-{}-x86_64.{}",
                download_url,
                version,
                if update_msi { "msi" } else { "exe" }
            )
        } else {
            format!("{}/rustdesk-{}-x86-sciter.exe", download_url, version)
        };
        log::debug!("New version available: {}", &version);
        let Some(file_path) = get_download_file_from_url(&download_url) else {
            bail!("Failed to get the file path from the URL: {}", download_url);
        };
        let expected_sha256 = download_file_expected_sha256(&download_url)?;
        let verify_res = ensure_verified_update_file(&download_url, &file_path, &expected_sha256);
        if verify_res.is_err() {
            clear_download_file_expected_sha256(&download_url);
        }
        verify_res?;
        // We have checked if the `conns` is empty before, but we need to check again.
        // No need to care about the downloaded file here, because it's rare case that the `conns` are empty
        // before the download, but not empty after the download.
        if has_no_active_conns() {
            #[cfg(target_os = "windows")]
            update_new_version(update_msi, &version, &file_path, &expected_sha256);
        }
    }
    Ok(())
}

fn ensure_verified_update_file(
    download_url: &str,
    file_path: &Path,
    expected_sha256: &str,
) -> ResultType<()> {
    let client = create_http_client_with_url(download_url, true);
    let mut is_file_exists = false;
    if file_path.exists() {
        // Check if the file size is the same as the server file size
        // If the file size is the same, we don't need to download it again.
        let file_size = std::fs::metadata(file_path)?.len();
        let response = client
            .head(download_url)
            .timeout(UPDATE_HTTP_REQUEST_TIMEOUT)
            .send()?;
        if !response.status().is_success() {
            bail!("Failed to get the file size: {}", response.status());
        }
        let total_size = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|ct_len| ct_len.to_str().ok())
            .and_then(|ct_len| ct_len.parse::<u64>().ok());
        let Some(total_size) = total_size else {
            bail!("Failed to get content length");
        };
        if file_size == total_size {
            match verify_file_sha256(file_path, expected_sha256) {
                Ok(()) => is_file_exists = true,
                Err(e) => {
                    log::warn!("Removing cached update file with invalid SHA256: {}", e);
                    std::fs::remove_file(file_path)?;
                }
            }
        } else {
            std::fs::remove_file(file_path)?;
        }
    }
    if !is_file_exists {
        let mut response = client.get(download_url).send()?;
        if !response.status().is_success() {
            bail!(
                "Failed to download the new version file: {}",
                response.status()
            );
        }
        write_verified_download_from_reader(file_path, &mut response, expected_sha256)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn verified_update_path(
    p: &str,
    expected_sha256: &str,
    kind: &str,
    file_path: &Path,
) -> Option<(crate::platform::VerifiedUpdateFile, String)> {
    let update_file =
        match crate::platform::verify_update_file_signature_and_sha256(p, expected_sha256) {
            Ok(update_file) => update_file,
            Err(e) => {
                log::error!("Refusing to update from untrusted {}: {}", kind, e);
                std::fs::remove_file(file_path).ok();
                return None;
            }
        };
    let update_path = match update_file.path_str() {
        Ok(path) => path.to_owned(),
        Err(e) => {
            log::error!("Failed to get verified {} path: {}", kind, e);
            std::fs::remove_file(file_path).ok();
            return None;
        }
    };
    Some((update_file, update_path))
}

#[cfg(target_os = "windows")]
fn update_new_version(update_msi: bool, version: &str, file_path: &PathBuf, expected_sha256: &str) {
    log::debug!(
        "New version is downloaded, update begin, update msi: {update_msi}, version: {version}, file: {:?}",
        file_path.to_str()
    );
    if let Some(p) = file_path.to_str() {
        if let Some(session_id) = crate::platform::get_current_process_session_id() {
            if update_msi {
                let Some((_update_file, update_path)) =
                    verified_update_path(p, expected_sha256, "msi", file_path)
                else {
                    return;
                };
                match crate::platform::update_me_msi(&update_path, true) {
                    Ok(_) => {
                        log::debug!("New version \"{}\" updated.", version);
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to install the new msi version  \"{}\": {}",
                            version,
                            e
                        );
                        std::fs::remove_file(&file_path).ok();
                    }
                }
            } else {
                let Some((_update_file, update_path)) =
                    verified_update_path(p, expected_sha256, "exe", file_path)
                else {
                    return;
                };
                let custom_client_staging_dir = if crate::is_custom_client() {
                    let custom_client_staging_dir =
                        crate::platform::get_custom_client_staging_dir();
                    if let Err(e) = crate::platform::handle_custom_client_staging_dir_before_update(
                        &custom_client_staging_dir,
                    ) {
                        log::error!(
                            "Failed to handle custom client staging dir before update: {}",
                            e
                        );
                        std::fs::remove_file(&file_path).ok();
                        return;
                    }
                    Some(custom_client_staging_dir)
                } else {
                    // Clean up any residual staging directory from previous custom client
                    let staging_dir = crate::platform::get_custom_client_staging_dir();
                    hbb_common::allow_err!(crate::platform::remove_custom_client_staging_dir(
                        &staging_dir
                    ));
                    None
                };
                let update_launched = match crate::platform::launch_privileged_process(
                    session_id,
                    &format!("\"{}\" --update", update_path),
                ) {
                    Ok(h) => {
                        if h.is_null() {
                            log::error!("Failed to update to the new version: {}", version);
                            false
                        } else {
                            log::debug!("New version \"{}\" is launched.", version);
                            true
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to run the new version: {}", e);
                        false
                    }
                };
                if !update_launched {
                    if let Some(dir) = custom_client_staging_dir {
                        hbb_common::allow_err!(crate::platform::remove_custom_client_staging_dir(
                            &dir
                        ));
                    }
                    std::fs::remove_file(&file_path).ok();
                }
            }
        } else {
            log::error!(
                "Failed to get the current process session id, Error {}",
                std::io::Error::last_os_error()
            );
            std::fs::remove_file(&file_path).ok();
        }
    } else {
        // unreachable!()
        log::error!(
            "Failed to convert the file path to string: {}",
            file_path.display()
        );
    }
}

pub fn get_download_file_from_url(url: &str) -> Option<PathBuf> {
    let filename = url.split('/').last()?;
    Some(std::env::temp_dir().join(filename))
}

fn create_download_temp_file(final_path: &Path) -> ResultType<(std::fs::File, PathBuf)> {
    let Some(download_dir) = final_path.parent() else {
        bail!(
            "Update file has no parent directory: {}",
            final_path.display()
        );
    };
    let Some(file_name) = final_path.file_name() else {
        bail!("Update file has no file name: {}", final_path.display());
    };
    let file_name = file_name.to_string_lossy();
    for _ in 0..16 {
        let temp_path = download_dir.join(format!(
            ".{}.{}.{}.download",
            file_name,
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((file, temp_path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    bail!("Failed to create temporary update file");
}

fn install_verified_download(temp_path: &Path, final_path: &Path) -> ResultType<()> {
    if std::fs::symlink_metadata(final_path).is_ok() {
        std::fs::remove_file(final_path)?;
    }
    if let Err(e) = std::fs::rename(temp_path, final_path) {
        std::fs::remove_file(temp_path).ok();
        return Err(e.into());
    }
    Ok(())
}

fn copy_and_verify_download_file<R: Read>(
    file: &mut std::fs::File,
    temp_path: &Path,
    reader: &mut R,
    expected_sha256: &str,
) -> ResultType<()> {
    std::io::copy(reader, file)?;
    file.flush()?;
    verify_file_sha256(temp_path, expected_sha256)
}

fn write_verified_download_from_reader<R: Read>(
    final_path: &Path,
    reader: &mut R,
    expected_sha256: &str,
) -> ResultType<()> {
    let (mut file, temp_path) = create_download_temp_file(final_path)?;
    if let Err(e) = copy_and_verify_download_file(&mut file, &temp_path, reader, expected_sha256) {
        std::fs::remove_file(temp_path).ok();
        return Err(e);
    }
    drop(file);
    if let Err(e) = install_verified_download(&temp_path, final_path) {
        std::fs::remove_file(temp_path).ok();
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
fn write_verified_download(
    final_path: &Path,
    file_data: &[u8],
    expected_sha256: &str,
) -> ResultType<()> {
    let mut reader = file_data;
    write_verified_download_from_reader(final_path, &mut reader, expected_sha256)
}

#[derive(serde::Deserialize)]
struct GithubRelease {
    assets: Vec<GithubReleaseAsset>,
}

#[derive(serde::Deserialize)]
struct GithubReleaseAsset {
    name: String,
    digest: Option<String>,
}

fn fetch_github_asset_sha256(
    release_or_download_url: &str,
    download_url: &str,
) -> ResultType<String> {
    let api_url = github_release_api_url(release_or_download_url)?;
    let asset_name = download_asset_name(download_url)?;
    let metadata = fetch_github_release_metadata(&api_url)?;
    github_release_asset_sha256(&metadata, &asset_name)
}

fn fetch_github_release_metadata(api_url: &str) -> ResultType<String> {
    let client = create_http_client_with_url(&api_url, true);
    let response = client
        .get(api_url)
        .header(reqwest::header::USER_AGENT, "rustdesk-updater")
        .timeout(UPDATE_HTTP_REQUEST_TIMEOUT)
        .send()?;
    if !response.status().is_success() {
        let status = response.status();
        if status == reqwest::StatusCode::FORBIDDEN
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            bail!(
                "Failed to get GitHub release metadata: {}. GitHub API rate limit may have been reached. Please retry later or download from the release page.",
                status
            );
        }
        bail!("Failed to get GitHub release metadata: {}", status);
    }
    Ok(response.text()?)
}

fn normalize_sha256_hex(sha256: &str) -> ResultType<String> {
    let sha256 = sha256.trim().to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("Update file SHA256 is malformed");
    }
    Ok(sha256)
}

fn cache_download_file_expected_sha256(
    download_url: &str,
    expected_sha256: &str,
) -> ResultType<String> {
    let expected_sha256 = normalize_sha256_hex(expected_sha256)?;
    DOWNLOAD_FILE_SHA256_CACHE
        .lock()
        .unwrap()
        .insert(download_url.to_owned(), expected_sha256.clone());
    Ok(expected_sha256)
}

fn cached_download_file_expected_sha256(download_url: &str) -> Option<String> {
    DOWNLOAD_FILE_SHA256_CACHE
        .lock()
        .unwrap()
        .get(download_url)
        .cloned()
}

pub fn clear_download_file_expected_sha256(download_url: &str) {
    DOWNLOAD_FILE_SHA256_CACHE
        .lock()
        .unwrap()
        .remove(download_url);
}

pub fn refresh_download_file_expected_sha256(download_url: &str) -> ResultType<String> {
    let expected_sha256 = fetch_github_asset_sha256(download_url, download_url)?;
    cache_download_file_expected_sha256(download_url, &expected_sha256)
}

pub fn download_file_expected_sha256(download_url: &str) -> ResultType<String> {
    match refresh_download_file_expected_sha256(download_url) {
        Ok(expected_sha256) => Ok(expected_sha256),
        Err(e) => {
            if let Some(expected_sha256) = cached_download_file_expected_sha256(download_url) {
                log::warn!(
                    "Failed to refresh update file SHA256 for {}, using cached value: {}",
                    download_url,
                    e
                );
                return Ok(expected_sha256);
            }
            Err(e)
        }
    }
}

fn github_release_api_url(update_url: &str) -> ResultType<String> {
    let url = reqwest::Url::parse(update_url)?;
    if url.scheme() != "https" || url.host_str() != Some("github.com") {
        bail!(
            "Update URL is not a GitHub HTTPS release URL: {}",
            update_url
        );
    }

    let Some(mut segments) = url.path_segments() else {
        bail!("GitHub update URL has no path: {}", update_url);
    };
    let Some(owner) = segments.next() else {
        bail!("GitHub update URL has no owner: {}", update_url);
    };
    let Some(repo) = segments.next() else {
        bail!("GitHub update URL has no repo: {}", update_url);
    };
    if owner != "rustdesk" || repo != "rustdesk" {
        bail!(
            "GitHub update URL is not a RustDesk release URL: {}",
            update_url
        );
    }
    if segments.next() != Some("releases") {
        bail!("GitHub update URL is not a release URL: {}", update_url);
    }

    let tag = match segments.next() {
        Some("tag") => segments.collect::<Vec<_>>().join("/"),
        Some("download") => {
            let Some(tag) = segments.next() else {
                bail!("GitHub update URL has no release tag: {}", update_url);
            };
            if segments.next().is_none() {
                bail!("GitHub update URL has no release asset: {}", update_url);
            }
            tag.to_owned()
        }
        _ => bail!(
            "GitHub update URL is not a release tag or download URL: {}",
            update_url
        ),
    };
    if tag.is_empty() {
        bail!("GitHub update URL has no release tag: {}", update_url);
    }

    Ok(format!(
        "https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}"
    ))
}

fn download_asset_name(download_url: &str) -> ResultType<String> {
    let Some(asset_name) = download_url.split('/').last() else {
        bail!("Download URL has no asset name: {}", download_url);
    };
    if asset_name.is_empty() {
        bail!("Download URL has empty asset name: {}", download_url);
    }
    Ok(asset_name.to_owned())
}

fn github_release_asset_sha256(release_json: &str, asset_name: &str) -> ResultType<String> {
    let release: GithubRelease = serde_json::from_str(release_json)?;
    let Some(asset) = release.assets.iter().find(|asset| asset.name == asset_name) else {
        bail!("GitHub release asset not found: {}", asset_name);
    };
    let Some(digest) = asset.digest.as_deref() else {
        bail!("GitHub release asset has no digest: {}", asset_name);
    };
    parse_sha256_digest(digest)
}

fn parse_sha256_digest(digest: &str) -> ResultType<String> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        bail!("GitHub release asset digest is not SHA256: {}", digest);
    };
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!(
            "GitHub release asset SHA256 digest is malformed: {}",
            digest
        );
    }
    Ok(hex.to_lowercase())
}

fn verify_file_sha256(path: &Path, expected_sha256: &str) -> ResultType<()> {
    let actual_sha256 = sha256_file_hex(path)?;
    if actual_sha256 != expected_sha256 {
        bail!(
            "SHA256 mismatch for {}: expected {}, got {}",
            path.display(),
            expected_sha256,
            actual_sha256
        );
    }
    Ok(())
}

fn sha256_file_hex(path: &Path) -> ResultType<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::default();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        sha2::Digest::update(&mut hasher, &buffer[..count]);
    }
    Ok(format!("{:x}", sha2::Digest::finalize(hasher)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_release_api_url_accepts_tag_url() {
        let api_url =
            github_release_api_url("https://github.com/rustdesk/rustdesk/releases/tag/1.4.3")
                .unwrap();

        assert_eq!(
            api_url,
            "https://api.github.com/repos/rustdesk/rustdesk/releases/tags/1.4.3"
        );
    }

    #[test]
    fn github_release_api_url_accepts_download_url() {
        let api_url = github_release_api_url(
            "https://github.com/rustdesk/rustdesk/releases/download/1.4.3/rustdesk-1.4.3-x86_64.exe",
        )
        .unwrap();

        assert_eq!(
            api_url,
            "https://api.github.com/repos/rustdesk/rustdesk/releases/tags/1.4.3"
        );
    }

    #[test]
    fn github_release_api_url_rejects_non_release_download_url() {
        assert!(github_release_api_url(
            "https://github.com/rustdesk/rustdesk/archive/refs/tags/1.4.3.zip"
        )
        .is_err());
    }

    #[test]
    fn github_release_api_url_rejects_non_github_url() {
        assert!(github_release_api_url("https://example.com/rustdesk/releases/tag/1.4.3").is_err());
    }

    #[test]
    fn github_release_api_url_rejects_non_rustdesk_repo() {
        assert!(
            github_release_api_url("https://github.com/other/rustdesk/releases/tag/1.4.3").is_err()
        );
        assert!(
            github_release_api_url("https://github.com/rustdesk/other/releases/tag/1.4.3").is_err()
        );
    }

    #[test]
    fn github_release_digest_requires_exact_asset_name_and_sha256_digest() {
        let json = r#"{
            "assets": [
                {"name": "rustdesk-1.4.3-x86_64.exe", "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},
                {"name": "rustdesk-1.4.3-x86_64.msi", "digest": "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"}
            ]
        }"#;

        let digest = github_release_asset_sha256(json, "rustdesk-1.4.3-x86_64.exe").unwrap();

        assert_eq!(
            digest,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn github_release_digest_rejects_missing_or_malformed_digest() {
        let missing = r#"{"assets": [{"name": "rustdesk.exe"}]}"#;
        let malformed = r#"{"assets": [{"name": "rustdesk.exe", "digest": "sha1:abcd"}]}"#;

        assert!(github_release_asset_sha256(missing, "rustdesk.exe").is_err());
        assert!(github_release_asset_sha256(malformed, "rustdesk.exe").is_err());
    }

    #[test]
    fn update_http_request_timeout_is_bounded() {
        assert_eq!(UPDATE_HTTP_REQUEST_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn download_file_sha256_cache_roundtrips_and_clears() {
        let download_url = format!(
            "https://github.com/rustdesk/rustdesk/releases/download/test/rustdesk-cache-test-{}-{}.exe",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        );
        let expected_sha256 = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
        clear_download_file_expected_sha256(&download_url);

        let cached = cache_download_file_expected_sha256(&download_url, expected_sha256).unwrap();

        assert_eq!(
            cached,
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
        assert_eq!(
            cached_download_file_expected_sha256(&download_url),
            Some(cached)
        );
        clear_download_file_expected_sha256(&download_url);
        assert_eq!(cached_download_file_expected_sha256(&download_url), None);
    }

    #[test]
    fn download_file_sha256_cache_rejects_malformed_digest() {
        let download_url = format!(
            "https://github.com/rustdesk/rustdesk/releases/download/test/rustdesk-cache-test-{}-{}.exe",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        );
        clear_download_file_expected_sha256(&download_url);

        assert!(cache_download_file_expected_sha256(&download_url, "sha256:not-hex").is_err());
        assert_eq!(cached_download_file_expected_sha256(&download_url), None);
    }

    #[test]
    fn verify_file_sha256_rejects_mismatched_file() {
        let file_path = std::env::temp_dir().join(format!(
            "rustdesk-updater-sha256-test-{}",
            std::process::id()
        ));
        std::fs::write(&file_path, b"rustdesk").unwrap();

        let result = verify_file_sha256(
            &file_path,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        std::fs::remove_file(&file_path).unwrap();

        assert!(result.is_err());
    }

    #[test]
    fn create_download_temp_file_uses_random_sibling_path() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-updater-temp-file-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&test_dir);
        std::fs::create_dir_all(&test_dir).unwrap();
        let final_path = test_dir.join("rustdesk-update.exe");

        let (file, temp_path) = create_download_temp_file(&final_path).unwrap();

        drop(file);
        let temp_file_name = temp_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap();
        assert_ne!(temp_path, final_path);
        assert_eq!(temp_path.parent(), Some(test_dir.as_path()));
        assert!(temp_file_name.starts_with(".rustdesk-update.exe."));
        assert!(temp_file_name.ends_with(".download"));
        assert!(temp_path.exists());
        std::fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn write_verified_download_removes_temp_file_on_sha256_error() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-updater-cleanup-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&test_dir);
        std::fs::create_dir_all(&test_dir).unwrap();
        let final_path = test_dir.join("rustdesk-update.exe");

        let result = write_verified_download(
            &final_path,
            b"update",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );

        assert!(result.is_err());
        assert!(!final_path.exists());
        assert!(std::fs::read_dir(&test_dir).unwrap().next().is_none());
        std::fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn write_verified_download_from_reader_installs_verified_file() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-updater-reader-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&test_dir);
        std::fs::create_dir_all(&test_dir).unwrap();
        let final_path = test_dir.join("rustdesk-update.exe");
        let mut data: &[u8] = b"update";

        write_verified_download_from_reader(
            &final_path,
            &mut data,
            "2937013f2181810606b2a799b05bda2849f3e369a20982a4138f0e0a55984ce4",
        )
        .unwrap();

        assert_eq!(std::fs::read(&final_path).unwrap(), b"update");
        std::fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn write_verified_download_removes_temp_file_on_install_error() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-updater-install-error-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&test_dir);
        std::fs::create_dir_all(&test_dir).unwrap();
        let final_path = test_dir.join("rustdesk-update.exe");
        std::fs::create_dir(&final_path).unwrap();

        let result = write_verified_download(
            &final_path,
            b"update",
            "2937013f2181810606b2a799b05bda2849f3e369a20982a4138f0e0a55984ce4",
        );

        assert!(result.is_err());
        assert!(final_path.is_dir());
        assert_eq!(std::fs::read_dir(&test_dir).unwrap().count(), 1);
        std::fs::remove_dir_all(&test_dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn install_verified_download_replaces_symlink_without_touching_target() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-updater-symlink-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&test_dir);
        std::fs::create_dir_all(&test_dir).unwrap();
        let final_path = test_dir.join("rustdesk-update.exe");
        let temp_path = test_dir.join(".rustdesk-update.exe.tmp");
        let victim_path = test_dir.join("victim");
        std::fs::write(&victim_path, b"victim").unwrap();
        std::os::unix::fs::symlink(&victim_path, &final_path).unwrap();
        std::fs::write(&temp_path, b"update").unwrap();

        install_verified_download(&temp_path, &final_path).unwrap();

        assert_eq!(std::fs::read(&victim_path).unwrap(), b"victim");
        assert_eq!(std::fs::read(&final_path).unwrap(), b"update");
        assert!(!std::fs::symlink_metadata(&final_path)
            .unwrap()
            .file_type()
            .is_symlink());
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
}
