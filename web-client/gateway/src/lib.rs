//! Browser-to-RustDesk gateway.
//!
//! The gateway speaks RustDesk's framed protobuf session to an unmodified host
//! and a small JSON-plus-binary WebSocket protocol to the browser. Video bytes
//! are forwarded as the host encoded them.

pub mod auth;
pub mod control;
pub mod direct;
pub mod handshake;
pub mod rendezvous;
pub mod server;
pub mod session;
pub mod video;

pub use hbb_common::ResultType;
