extern crate embed_resource;

fn main() {
    println!("cargo:rustc-link-lib=dylib:+verbatim=./Runner.res");
}
