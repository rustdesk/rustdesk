mod license;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hbb_common::ResultType;
use license::*;

fn gen_name(lic: &License) -> ResultType<String> {
    let tmp = URL_SAFE_NO_PAD.encode(&serde_json::to_vec(lic)?);
    Ok(tmp.chars().rev().collect())
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let api = args.get(2).cloned().unwrap_or_default();
    if args.len() >= 2 {
        println!(
            "rustdesk-licensed-{}.exe",
            gen_name(&License {
                key: args[0].clone(),
                host: args[1].clone(),
                api,
            })
            .unwrap()
        );
    }
    if args.len() == 1 {
        println!("{:?}", get_license_from_string(&args[0]));
    }
}
