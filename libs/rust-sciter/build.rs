#[cfg(windows)]
fn main() {
    println!("cargo:rustc-link-search=native=./");
    println!("cargo:rustc-link-lib=static=sciter.static");
    println!("cargo:rustc-link-lib=comdlg32");
    println!("cargo:rustc-link-lib=wininet");
    println!("cargo:rustc-link-lib=windowscodecs");
}

#[cfg(not(windows))]
fn main() {}
