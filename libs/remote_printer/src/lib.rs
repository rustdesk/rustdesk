#[cfg(target_os = "windows")]
pub mod pdf;
#[cfg(target_os = "windows")]
pub use pdf::is_installed as is_rd_printer_installed;
