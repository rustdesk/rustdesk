use std::{
    cell::RefCell,
    io::{self, Read},
};
use zstd::bulk::Compressor;

const MAX_DECOMPRESSED_SIZE: usize = 256 * 1024 * 1024;

// The library supports regular compression levels from 1 up to ZSTD_maxCLevel(),
// which is currently 22. Levels >= 20
// Default level is ZSTD_CLEVEL_DEFAULT==3.
// value 0 means default, which is controlled by ZSTD_CLEVEL_DEFAULT
thread_local! {
    static COMPRESSOR: RefCell<io::Result<Compressor<'static>>> = RefCell::new(Compressor::new(crate::config::COMPRESS_LEVEL));
}

pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    COMPRESSOR.with(|c| {
        if let Ok(mut c) = c.try_borrow_mut() {
            match &mut *c {
                Ok(c) => match c.compress(data) {
                    Ok(res) => out = res,
                    Err(err) => {
                        crate::log::debug!("Failed to compress: {}", err);
                    }
                },
                Err(err) => {
                    crate::log::debug!("Failed to get compressor: {}", err);
                }
            }
        }
    });
    out
}

pub fn decompress(data: &[u8]) -> Vec<u8> {
    decompress_with_limit(data, MAX_DECOMPRESSED_SIZE).unwrap_or_default()
}

fn decompress_with_limit(data: &[u8], limit: usize) -> io::Result<Vec<u8>> {
    let decoder = zstd::Decoder::new(data)?;
    let mut output = Vec::new();
    decoder
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut output)?;
    if output.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "decompressed data exceeds size limit",
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_data_larger_than_limit() {
        let compressed = zstd::encode_all(&vec![0u8; 1025][..], 0).unwrap();
        assert!(decompress_with_limit(&compressed, 1024).is_err());
    }

    #[test]
    fn accepts_data_at_limit() {
        let input = vec![0u8; 1024];
        let compressed = zstd::encode_all(&input[..], 0).unwrap();
        assert_eq!(
            decompress_with_limit(&compressed, input.len()).unwrap(),
            input
        );
    }
}
