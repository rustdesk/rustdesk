mod setup;
pub use setup::{
    is_rd_printer_installed,
    setup::{install_update_printer, uninstall_printer},
};

const RD_DRIVER_INF_PATH: &str = "drivers/RustDeskPrinterDriver/RustDeskPrinterDriver.inf";

fn get_printer_name(app_name: &str) -> Vec<u16> {
    format!("{} Printer", app_name)
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

fn get_driver_name() -> Vec<u16> {
    "RustDesk v4 Printer Driver"
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

fn get_port_name(app_name: &str) -> Vec<u16> {
    format!("{} Printer", app_name)
        .encode_utf16()
        .chain(Some(0))
        .collect()
}
