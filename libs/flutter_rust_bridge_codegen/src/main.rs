use env_logger::Env;
use log::info;
use structopt::StructOpt;

use lib_flutter_rust_bridge_codegen::{frb_codegen, Opts};

fn main() {
    let opts = Opts::from_args();
    env_logger::Builder::from_env(Env::default().default_filter_or(if opts.verbose {
        "debug"
    } else {
        "info"
    }))
    .init();

    frb_codegen(opts).unwrap();

    info!("Now go and use it :)");
}
