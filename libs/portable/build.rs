extern crate embed_resource;
use std::fs;

fn main() {
    let runner_res_path = "Runner.res";
    match fs::metadata(runner_res_path) {
        Ok(_) => println!("cargo:rustc-link-lib=dylib:+verbatim=./libs/portable/Runner.res"),
        Err(_) => embed_resource::compile("icon.rc", embed_resource::NONE),
    }
}
