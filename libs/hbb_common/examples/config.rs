extern crate hbb_common;

fn main() {
    println!("{:?}", hbb_common::config::PeerConfig::load("455058072"));
}
