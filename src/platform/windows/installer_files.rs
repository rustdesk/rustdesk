use hbb_common::{
    anyhow::{anyhow, Context},
    base64::{engine::general_purpose::STANDARD, Engine as _},
    ResultType,
};
use std::fs;

pub(super) const VERIFY_FILE_ARGUMENT: &str = "--verify-install-file";
pub(super) const DECODE_SHORTCUT_ARGUMENT: &str = "--decode-install-shortcut";

pub(crate) fn handle_command(args: &[String]) -> Option<ResultType<()>> {
    match args.first()?.as_str() {
        VERIFY_FILE_ARGUMENT => Some(match args {
            [_, source, expected_hash, receipt] => verify(source, expected_hash, receipt),
            _ => Err(anyhow!(
                "Expected a file, SHA-256 hash and verification receipt path"
            )),
        }),
        DECODE_SHORTCUT_ARGUMENT => Some(match args {
            [_, source, destination] => decode_shortcut(source, destination),
            _ => Err(anyhow!("Expected encoded shortcut and destination paths")),
        }),
        _ => None,
    }
}

fn verify(source: &str, expected_hash: &str, receipt: &str) -> ResultType<()> {
    let mut file = fs::File::open(source)
        .with_context(|| format!("Failed to open installer file {source}"))?;
    crate::update_hash::verify_sha256_reader(&mut file, expected_hash)
        .map_err(|error| anyhow!("Failed to verify installer file {source}: {error:?}"))?;
    // Startup can exit before dispatch; the caller requires proof that verification ran.
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(receipt)
        .with_context(|| format!("Failed to write verification receipt {receipt}"))?;
    Ok(())
}

fn decode_shortcut(source: &str, destination: &str) -> ResultType<()> {
    let encoded = fs::read_to_string(source)
        .with_context(|| format!("Failed to read encoded installer shortcut {source}"))?;
    let bytes = STANDARD
        .decode(encoded.trim())
        .context("Failed to decode installer shortcut")?;
    fs::write(destination, bytes)
        .with_context(|| format!("Failed to write installer shortcut {destination}"))
}
