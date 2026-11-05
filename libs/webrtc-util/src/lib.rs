#![warn(rust_2018_idioms)]
#![allow(dead_code)]

use std::io;

use async_trait::async_trait;
use thiserror::Error;

#[cfg(feature = "vnet")]
#[macro_use]
extern crate lazy_static;

#[cfg(target_family = "windows")]
#[macro_use]
extern crate bitflags;

pub mod fixed_big_int;
pub mod replay_detector;

/// KeyingMaterialExporter to extract keying material.
///
/// This trait sits here to avoid getting a direct dependency between
/// the dtls and srtp crates.
#[async_trait]
pub trait KeyingMaterialExporter {
    async fn export_keying_material(
        &self,
        label: &str,
        context: &[u8],
        length: usize,
    ) -> std::result::Result<Vec<u8>, KeyingMaterialExporterError>;
}

/// Possible errors while exporting keying material.
///
/// These errors might have been more logically kept in the dtls
/// crate, but that would have required a direct dependency between
/// srtp and dtls.
#[derive(Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum KeyingMaterialExporterError {
    #[error("tls handshake is in progress")]
    HandshakeInProgress,
    #[error("context is not supported for export_keying_material")]
    ContextUnsupported,
    #[error("export_keying_material can not be used with a reserved label")]
    ReservedExportKeyingMaterial,
    #[error("no cipher suite for export_keying_material")]
    CipherSuiteUnset,
    #[error("export_keying_material io: {0}")]
    Io(#[source] error::IoError),
    #[error("export_keying_material hash: {0}")]
    Hash(String),
}

impl From<io::Error> for KeyingMaterialExporterError {
    fn from(e: io::Error) -> Self {
        KeyingMaterialExporterError::Io(error::IoError(e))
    }
}

#[cfg(feature = "buffer")]
pub mod buffer;

#[cfg(feature = "conn")]
pub mod conn;

#[cfg(feature = "ifaces")]
pub mod ifaces;

#[cfg(feature = "vnet")]
pub mod vnet;

#[cfg(feature = "marshal")]
pub mod marshal;

#[cfg(feature = "buffer")]
pub use crate::buffer::Buffer;
#[cfg(feature = "conn")]
pub use crate::conn::Conn;
#[cfg(feature = "marshal")]
pub use crate::marshal::{exact_size_buf::ExactSizeBuf, Marshal, MarshalSize, Unmarshal};

mod error;
pub use error::{Error, Result};

#[cfg(feature = "sync")]
pub mod sync;
