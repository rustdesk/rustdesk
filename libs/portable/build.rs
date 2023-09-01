extern crate embed_resource;

fn main() {
    println!("cargo:rustc-link-lib=dylib:+verbatim=./flutter/build/windows/runner/rustdesk.dir/Release/Runner.res");
}
