#[cfg(target_os = "windows")]
mod setup;
#[cfg(target_os = "windows")]
pub use setup::{
    is_rd_printer_installed,
    setup::{install_update_printer, uninstall_printer},
};

#[cfg(target_os = "windows")]
const RD_DRIVER_INF_PATH: &str = "drivers/RustDeskPrinterDriver/RustDeskPrinterDriver.inf";

#[cfg(target_os = "windows")]
fn get_printer_name(app_name: &str) -> Vec<u16> {
    format!("{} Printer", app_name)
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn get_driver_name() -> Vec<u16> {
    "RustDesk v4 Printer Driver"
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn get_port_name(app_name: &str) -> Vec<u16> {
    format!("{} Printer", app_name)
        .encode_utf16()
        .chain(Some(0))
        .collect()
}
