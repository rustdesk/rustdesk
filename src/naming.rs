mod license;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hbb_common::ResultType;
use license::*;

fn gen_name(lic: &License) -> ResultType<String> {
    let tmp = serde_json::to_vec::<License>(lic)?;
    let tmp = URL_SAFE_NO_PAD.encode(&tmp);
    let tmp: String = tmp.chars().rev().collect();
    Ok(tmp)
}

fn main() {
    let mut args = Vec::new();
    let mut i = 0;
    for arg in std::env::args() {
        if i > 0 {
            args.push(arg);
        }
        i += 1;
    }
    let api = if args.len() < 3 {
        "".to_owned()
    } else {
        args[2].clone()
    };
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
